//! Icon pipeline (P2.1-E2/E3/E4, review 79 §6-21): extraction → L1 memory
//! LRU (byte-budgeted) → L2 PNG disk cache (atomic, corrupt-recoverable).
//!
//! Boundaries (INV-ICON-001/007): HICON/HBITMAP/GDI live ONLY in this file;
//! everything above sees decoded RGBA buffers keyed by `IconKey`. The cache
//! is intentionally serialized behind one mutex — "at most one active
//! extraction per key" (INV-ICON-003) holds trivially at v0.1 scale.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use launcher_domain::IconKey;

pub const EXTRACTOR_VERSION: u32 = 1;

/// Decoded icon pixels (review 79 §10): RGBA, top-down, non-premultiplied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconBitmap {
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
}

impl IconBitmap {
    pub fn byte_len(&self) -> usize {
        (self.width as usize) * (self.height as usize) * 4
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IconError {
    #[error("icon extraction failed: {0}")]
    Extract(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// L1 memory cache: LRU with a HARD BYTE budget (review 79 §9/§45/§46).
/// Eviction drives usage down to 80% of the budget to avoid thrashing.
pub struct IconMemoryCache {
    budget_bytes: usize,
    entries: HashMap<String, (IconBitmap, u64)>,
    clock: u64,
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    used_bytes: usize,
}

const EVICT_TARGET_PCT: usize = 80;

impl IconMemoryCache {
    pub fn new(budget_bytes: usize) -> Self {
        Self {
            budget_bytes: budget_bytes.max(1),
            entries: HashMap::new(),
            clock: 0,
            hits: 0,
            misses: 0,
            evictions: 0,
            used_bytes: 0,
        }
    }

    pub fn used_bytes(&self) -> usize {
        self.used_bytes
    }

    pub fn get(&mut self, key: &IconKey) -> Option<IconBitmap> {
        let k = key.cache_key();
        if let Some(entry) = self.entries.get_mut(&k) {
            self.clock += 1;
            entry.1 = self.clock;
            self.hits += 1;
            return Some(entry.0.clone());
        }
        self.misses += 1;
        None
    }

    pub fn put(&mut self, key: &IconKey, bitmap: IconBitmap) {
        let k = key.cache_key();
        if let Some(existing) = self.entries.remove(&k) {
            self.used_bytes -= existing.0.byte_len();
        }
        // a single bitmap bigger than the whole budget is never cached
        if bitmap.byte_len() > self.budget_bytes {
            return;
        }
        self.clock += 1;
        self.used_bytes += bitmap.byte_len();
        let stamp = self.clock;
        self.entries.insert(k, (bitmap, stamp));
        // evict least-recently-used until usage <= 80% budget
        while self.used_bytes > self.budget_bytes
            && self.used_bytes > self.budget_bytes * EVICT_TARGET_PCT / 100
        {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, (_, ts))| *ts)
                .map(|(k, _)| k.clone());
            let Some(oldest) = oldest else { break };
            if let Some((bmp, _)) = self.entries.remove(&oldest) {
                self.used_bytes -= bmp.byte_len();
                self.evictions += 1;
            }
        }
    }
}

/// L2 disk cache: one PNG per cache key, written atomically (review 79
/// §16/§19/§20). Corrupt entries are deleted on read (INV-ICON-005).
pub struct IconDiskCache {
    dir: PathBuf,
}

impl IconDiskCache {
    pub fn new(dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&dir);
        Self { dir }
    }

    fn path_for(&self, key: &IconKey) -> PathBuf {
        // stable, filesystem-safe name bound to the full cache key
        let name: String = key
            .cache_key()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        // cheap collision guard: FNV-1a of the full cache key
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in key.cache_key().as_bytes() {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0000_0100_0000_01B3);
        }
        self.dir
            .join(format!("{}_{h:016x}.png", &name[..name.len().min(40)]))
    }

    pub fn get(&self, key: &IconKey) -> Option<IconBitmap> {
        let path = self.path_for(key);
        let bytes = std::fs::read(&path).ok()?;
        match decode_png(&bytes) {
            Some(bmp) => Some(bmp),
            None => {
                // INV-ICON-005: corrupt entry → delete, caller re-extracts
                let _ = std::fs::remove_file(&path);
                None
            }
        }
    }

    pub fn put(&self, key: &IconKey, bmp: &IconBitmap) -> std::io::Result<()> {
        let mut png_bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png_bytes, bmp.width, bmp.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header()?;
            writer.write_image_data(&bmp.rgba)?;
        }
        // atomic: temp + flush + rename (review 79 §20)
        let tmp = self.dir.join(format!(
            ".tmp_{}_{:x}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        {
            use std::io::Write;
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(&png_bytes)?;
            f.flush()?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, self.path_for(key))?;
        Ok(())
    }
}

fn decode_png(bytes: &[u8]) -> Option<IconBitmap> {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().ok()?;
    if reader.output_color_type() != (png::ColorType::Rgba, png::BitDepth::Eight) {
        return None;
    }
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    Some(IconBitmap {
        width: info.width,
        height: info.height,
        rgba: buf[..info.buffer_size()].into(),
    })
}

/// What the extractor needs to fetch an icon for one identity.
#[derive(Debug, Clone)]
pub struct IconRequest {
    pub key: IconKey,
    /// Filesystem path of the icon source (executable or shortcut). Packaged
    /// aliases (`shell:...`) are not extractable at v0.1 — they fall through
    /// to the fallback icon (never a failure).
    pub source_path: PathBuf,
}

/// The service facade (review 79 §55): decoded RGBA from L1 → L2 →
/// extraction. Extraction is serialized behind the service lock — one
/// active extraction per key by construction.
pub struct IconService {
    memory: Mutex<IconMemoryCache>,
    disk: Option<IconDiskCache>,
    extract_count: AtomicU32,
    /// serializes extraction (INV-ICON-003)
    inner: Mutex<()>,
}

impl IconService {
    pub fn new(memory_budget_bytes: usize, disk_dir: Option<PathBuf>) -> Self {
        Self {
            memory: Mutex::new(IconMemoryCache::new(memory_budget_bytes)),
            disk: disk_dir.map(IconDiskCache::new),
            extract_count: AtomicU32::new(0),
            inner: Mutex::new(()),
        }
    }

    pub fn memory_used_bytes(&self) -> usize {
        self.memory.lock().expect("icon lru").used_bytes()
    }

    pub fn extractions(&self) -> u32 {
        self.extract_count.load(Ordering::SeqCst)
    }

    pub fn get(&self, request: &IconRequest) -> Result<IconBitmap, IconError> {
        let _guard = self.inner.lock().expect("icon service lock");
        if let Some(bmp) = self.memory.lock().expect("icon lru").get(&request.key) {
            return Ok(bmp);
        }
        if let Some(disk) = &self.disk {
            if let Some(bmp) = disk.get(&request.key) {
                self.memory
                    .lock()
                    .expect("icon lru")
                    .put(&request.key, bmp.clone());
                return Ok(bmp);
            }
        }
        let bmp = extract_icon(&request.source_path)?;
        self.extract_count.fetch_add(1, Ordering::SeqCst);
        if let Some(disk) = &self.disk {
            let _ = disk.put(&request.key, &bmp); // cache write failure is benign
        }
        self.memory
            .lock()
            .expect("icon lru")
            .put(&request.key, bmp.clone());
        Ok(bmp)
    }
}

/// Win32 extraction: SHGetFileInfoW → HICON → GetIconInfo → GetDIBits → RGBA
/// (base icon only — no shell overlays, review 79 §30). HICON/GDI handles
/// never leave this function (INV-ICON-007).
#[cfg(windows)]
fn extract_icon(source: &Path) -> Result<IconBitmap, IconError> {
    use windows::core::HSTRING;
    use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
    use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

    let path = HSTRING::from(source.as_os_str());
    let mut info = SHFILEINFOW::default();
    let res = unsafe {
        SHGetFileInfoW(
            &path,
            Default::default(),
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        )
    };
    if res == 0 || info.hIcon.is_invalid() {
        return Err(IconError::Extract(format!(
            "no icon for {}",
            source.display()
        )));
    }
    let hicon = info.hIcon;
    let bmp_result = (|| -> Result<IconBitmap, IconError> {
        unsafe {
            let mut ii = ICONINFO::default();
            GetIconInfo(hicon, &mut ii)
                .map_err(|e| IconError::Extract(format!("GetIconInfo: {e}")))?;
            let out = extract_bitmap_rgba(ii.hbmColor);
            let _ = windows::Win32::Graphics::Gdi::DeleteObject(ii.hbmColor);
            let _ = windows::Win32::Graphics::Gdi::DeleteObject(ii.hbmMask);
            out
        }
    })();
    unsafe {
        let _ = DestroyIcon(hicon);
    }
    bmp_result
}

#[cfg(windows)]
fn extract_bitmap_rgba(
    hbm: windows::Win32::Graphics::Gdi::HBITMAP,
) -> Result<IconBitmap, IconError> {
    use windows::Win32::Graphics::Gdi::{
        GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER,
        DIB_RGB_COLORS, BI_RGB,
    };
    unsafe {
        let mut bm = BITMAP::default();
        if GetObjectW(
            hbm,
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bm as *mut BITMAP as *mut _),
        ) == 0
        {
            return Err(IconError::Extract("GetObjectW failed".into()));
        }
        let (w, h) = (bm.bmWidth as u32, bm.bmHeight as u32);
        if w == 0 || h == 0 {
            return Err(IconError::Extract("empty icon bitmap".into()));
        }
        let mut bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w as i32,
                biHeight: h as i32, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        let hdc = GetDC(None);
        let copied = GetDIBits(
            hdc,
            hbm,
            0,
            h,
            Some(pixels.as_mut_ptr().cast()),
            &mut bmi,
            DIB_RGB_COLORS,
        );
        ReleaseDC(None, hdc);
        if copied == 0 {
            return Err(IconError::Extract("GetDIBits failed".into()));
        }
        // BGRA → RGBA; ensure opaque alpha when the 32bpp path leaves 0
        for px in pixels.chunks_exact_mut(4) {
            px.swap(0, 2);
            if px[3] == 0 {
                px[3] = 0xFF;
            }
        }
        Ok(IconBitmap {
            width: w,
            height: h,
            rgba: pixels.into(),
        })
    }
}

#[cfg(not(windows))]
fn extract_icon(_source: &Path) -> Result<IconBitmap, IconError> {
    Err(IconError::Extract("icon extraction requires windows".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_domain::IconVariant;

    fn key(app: &str) -> IconKey {
        IconKey {
            application: app.into(),
            variant: IconVariant::Normal,
            source_revision: 1,
            extractor_version: EXTRACTOR_VERSION,
        }
    }

    fn bmp(size: u32, fill: u8) -> IconBitmap {
        IconBitmap {
            width: size,
            height: size,
            rgba: vec![fill; (size * size * 4) as usize].into(),
        }
    }

    /// E3/ICON-008: byte-budget LRU evicts least-recently-used first and
    /// never exceeds the budget.
    #[test]
    fn memory_cache_budget_eviction() {
        let mut cache = IconMemoryCache::new(10_000);
        let k1 = key("app:one");
        let k2 = key("app:two");
        let k3 = key("app:three");
        cache.put(&k1, bmp(32, 1)); // 4096 bytes
        cache.put(&k2, bmp(32, 2)); // 8192 used
        assert!(cache.get(&k1).is_some()); // touch k1 → k2 becomes LRU
        cache.put(&k3, bmp(32, 3)); // forces eviction of k2
        assert!(cache.get(&k2).is_none(), "LRU entry must be evicted");
        assert!(cache.get(&k1).is_some(), "recently used entry survives");
        assert!(cache.get(&k3).is_some());
        assert!(cache.used_bytes() <= 10_000);
    }

    /// Oversized bitmaps are never cached (would blow the budget alone).
    #[test]
    fn oversized_bitmap_not_cached() {
        let mut cache = IconMemoryCache::new(1_000);
        cache.put(&key("big"), bmp(64, 1)); // 16KB >> 1KB budget
        assert!(cache.get(&key("big")).is_none());
        assert_eq!(cache.used_bytes(), 0);
    }

    /// E4/ICON-003/ICON-005: disk roundtrip + corrupt entry recovery.
    #[test]
    fn disk_cache_roundtrip_and_corrupt_recovery() {
        let dir = std::env::temp_dir().join(format!("nl_icon_disk_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let disk = IconDiskCache::new(dir.clone());
        let k = key("app:disk");
        let bitmap = bmp(24, 7);
        disk.put(&k, &bitmap).unwrap();
        let got = disk.get(&k).expect("roundtrip must succeed");
        assert_eq!(
            (got.width, got.height, got.rgba[..4].to_vec()),
            (24, 24, vec![7, 7, 7, 7])
        );

        // corrupt the file: get() deletes it and returns None (no panic)
        let p = disk.path_for(&k);
        std::fs::write(&p, b"not a png").unwrap();
        assert!(disk.get(&k).is_none(), "corrupt entry must be dropped");
        assert!(!p.exists(), "corrupt file must be removed");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// E2/ICON-011 (live): extract a real icon from a system executable;
    /// second get must come from cache (extraction count stays 1).
    #[cfg(windows)]
    #[test]
    fn live_extraction_and_caching() {
        let notepad = PathBuf::from(r"C:\Windows\System32\notepad.exe");
        if !notepad.exists() {
            return; // environment guard
        }
        let dir = std::env::temp_dir().join(format!("nl_icon_live_{}", std::process::id()));
        let svc = IconService::new(4 * 1024 * 1024, Some(dir.clone()));
        let k = IconKey {
            application: "win32:c:/windows/system32/notepad.exe".into(),
            variant: IconVariant::Normal,
            source_revision: 1,
            extractor_version: EXTRACTOR_VERSION,
        };
        let req = IconRequest {
            key: k.clone(),
            source_path: notepad,
        };
        let first = svc.get(&req).expect("system exe must have an icon");
        assert!(first.width > 0 && first.rgba.len() == first.byte_len());
        let second = svc.get(&req).expect("cached");
        assert_eq!(first, second);
        assert_eq!(svc.extractions(), 1, "second get must come from cache");
        assert!(svc.memory_used_bytes() > 0);
        std::fs::remove_dir_all(&dir).ok();
    }
}
