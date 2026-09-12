//! Management window visual capture (settings parity verification).
//!
//! `cargo run -p launcher-app --example management_shot -- <dir>` creates
//! the ManagementWindow with deterministic demo options (groups, toggle,
//! segmented, slider, hotkey row, info rows), shows it, captures the client
//! area via GDI PrintWindow in dark AND light palettes, writes BMPs into
//! <dir>, and quits. Windows-only; other platforms exit silently.

use std::path::PathBuf;

use slint::ComponentHandle as _;

fn demo_items() -> slint::ModelRc<launcher_ui::SettingItem> {
    use launcher_ui::SettingItem;
    let group = |label: &str| SettingItem {
        key: label.into(),
        label: label.into(),
        desc: "".into(),
        kind: "group".into(),
        value_str: "".into(),
        value_bool: false,
        value_num: 0,
        min: 0,
        max: 0,
        options: slint::ModelRc::default(),
        enabled: true,
        capturing: false,
        focus_ix: -1,
    };
    let item = |key: &str,
                label: &str,
                desc: &str,
                kind: &str,
                value_str: &str,
                value_bool: bool,
                value_num: i32,
                min: i32,
                max: i32,
                options: Vec<&str>,
                enabled: bool,
                capturing: bool,
                focus_ix: i32|
     -> SettingItem {
        SettingItem {
            key: key.into(),
            label: label.into(),
            desc: desc.into(),
            kind: kind.into(),
            value_str: value_str.into(),
            value_bool,
            value_num,
            min,
            max,
            options: slint::ModelRc::new(std::rc::Rc::new(slint::VecModel::from(
                options.into_iter().map(|s| s.into()).collect::<Vec<_>>(),
            ))),
            enabled,
            capturing,
            focus_ix,
        }
    };
    let items = vec![
        group("外观"),
        item("theme_mode", "主题模式", "弹窗与管理窗口配色，切换立即生效", "segmented", "system", false, 0, 0, 0, vec!["system", "light", "dark"], true, false, 0),
        group("行为"),
        item("autostart", "开机自启", "登录时自动启动 Native Launcher", "toggle", "", true, 0, 0, 0, vec![], true, false, 1),
        group("性能"),
        item("watch_enabled", "文件监视", "索引目录变化时增量更新（关闭后按周期重建）", "toggle", "", false, 0, 0, 0, vec![], true, false, 2),
        item("result_limit", "结果条数", "搜索结果最多显示的行数（重启后生效）", "slider", "", false, 12, 4, 16, vec![], true, false, 3),
        group("快捷键"),
        item("hotkey", "呼出热键", "点击输入框后按下新组合（至少一个修饰键，Esc 取消）", "hotkey", "Ctrl+Space", false, 0, 0, 0, vec![], true, false, 4),
        group("索引目录"),
        item("index_dirs", "已配置索引目录", "删除：搜索 settings index；添加：编辑 config.toml", "info", "(默认: 文档 / 桌面 / 下载)", false, 0, 0, 0, vec![], true, false, -1),
        item("ai", "AI Agent", "状态与配置（allow_remote_data）", "info", "未配置（在 config.toml 添加 [llm]）", false, 0, 0, 0, vec![], true, false, -1),
    ];
    slint::ModelRc::new(std::rc::Rc::new(slint::VecModel::from(items)))
}

fn find_window(title: &str) -> Option<isize> {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::FindWindowW;
        FindWindowW(None, &windows::core::HSTRING::from(title))
            .ok()
            .map(|h| h.0 as isize)
    }
    #[cfg(not(windows))]
    {
        let _ = title;
        None
    }
}

#[cfg(windows)]
fn capture_client_to_bmp(hwnd: isize) -> Option<Vec<u8>> {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
        ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetClientRect;
    #[link(name = "user32")]
    extern "system" {
        fn PrintWindow(hwnd: isize, hdc: isize, flags: u32) -> i32;
    }
    const PW_RENDERFULLCONTENT: u32 = 2;
    unsafe {
        let hwnd = HWND(hwnd as *mut _);
        let hdc_window = GetDC(hwnd);
        let mut rect = RECT::default();
        GetClientRect(hwnd, &mut rect).ok()?;
        let w = (rect.right - rect.left) as i32;
        let h = (rect.bottom - rect.top) as i32;
        if w <= 0 || h <= 0 {
            ReleaseDC(hwnd, hdc_window);
            return None;
        }
        let hdc_mem = CreateCompatibleDC(hdc_window);
        let hbmp = CreateCompatibleBitmap(hdc_window, w, h);
        let old = SelectObject(hdc_mem, hbmp);
        let _ = PrintWindow(hwnd.0 as isize, hdc_mem.0 as isize, PW_RENDERFULLCONTENT);
        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = w;
        bmi.bmiHeader.biHeight = h;
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB.0;
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        let got = GetDIBits(
            hdc_mem, hbmp, 0, h as u32, Some(pixels.as_mut_ptr() as *mut _), &mut bmi, DIB_RGB_COLORS,
        );
        SelectObject(hdc_mem, old);
        let _ = DeleteObject(hbmp);
        let _ = DeleteDC(hdc_mem);
        ReleaseDC(hwnd, hdc_window);
        if got == 0 {
            return None;
        }
        let data_size = pixels.len();
        let file_size = 54 + data_size;
        let mut bmp = Vec::with_capacity(file_size);
        bmp.extend_from_slice(b"BM");
        bmp.extend_from_slice(&(file_size as u32).to_le_bytes());
        bmp.extend_from_slice(&0u32.to_le_bytes());
        bmp.extend_from_slice(&54u32.to_le_bytes());
        bmp.extend_from_slice(&(40u32).to_le_bytes());
        bmp.extend_from_slice(&(w as i32).to_le_bytes());
        bmp.extend_from_slice(&(h as i32).to_le_bytes());
        bmp.extend_from_slice(&(1u16).to_le_bytes());
        bmp.extend_from_slice(&(32u16).to_le_bytes());
        bmp.extend_from_slice(&0u32.to_le_bytes());
        bmp.extend_from_slice(&(data_size as u32).to_le_bytes());
        bmp.extend_from_slice(&0u32.to_le_bytes());
        bmp.extend_from_slice(&0u32.to_le_bytes());
        bmp.extend_from_slice(&0u32.to_le_bytes());
        bmp.extend_from_slice(&0u32.to_le_bytes());
        bmp.extend_from_slice(&pixels);
        Some(bmp)
    }
}

#[cfg(not(windows))]
fn capture_client_to_bmp(_hwnd: isize) -> Option<Vec<u8>> {
    None
}

fn main() {
    let dir: PathBuf = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".openclaw-mgmt-shot"));
    let _ = std::fs::create_dir_all(&dir);

    let w = launcher_ui::ManagementWindow::new().expect("management window");
    let weak = w.as_weak();
    w.set_tab(0);
    w.set_setting_items(demo_items());
    w.set_save_ok(true);
    w.set_save_status("已保存 · theme_mode = system".into());
    let _ = w.show();

    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(900));
        let pass = |light: bool, name: &str, weak: slint::Weak<launcher_ui::ManagementWindow>| {
            let weak2 = weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = weak2.upgrade() {
                    w.global::<launcher_ui::Theme>().set_light_theme(light);
                }
            });
            std::thread::sleep(std::time::Duration::from_millis(700));
            if let Some(hwnd) = find_window("Launcher Management") {
                if let Some(bmp) = capture_client_to_bmp(hwnd) {
                    let path = dir.join(format!("{name}.bmp"));
                    match std::fs::write(&path, &bmp) {
                        Ok(()) => println!("captured {}", path.display()),
                        Err(e) => eprintln!("write failed: {e}"),
                    }
                } else {
                    eprintln!("capture failed for {name}");
                }
            } else {
                eprintln!("window not found for {name}");
            }
        };
        pass(false, "management-dark", weak.clone());
        pass(true, "management-light", weak.clone());
        let _ = slint::invoke_from_event_loop(|| {
            slint::quit_event_loop();
        });
    });
    slint::run_event_loop().expect("event loop");
}