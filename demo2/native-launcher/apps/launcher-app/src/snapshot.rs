//! Visual Regression baseline capture (Spec implementation step 8, section 51).
//!
//! `LAUNCHER_SNAPSHOT=<file.bmp>` runs the launcher with a deterministic demo
//! dataset (results / actions / workflow steps), shows the window at the
//! frozen 640x420 baseline, captures the client area via GDI, and writes a
//! BMP for pixel comparison. Windows-only; other platforms exit silently.

use std::path::PathBuf;

use launcher_domain::{Category, Command};

/// Deterministic demo dataset for the visual baseline (Spec section 51).
pub fn demo_data() -> (
    Vec<launcher_domain::Command>,
    Vec<launcher_ui::ActionItem>,
    Vec<launcher_ui::WorkflowItem>,
) {
    let results: Vec<Command> = (0..3)
        .map(|i| Command {
            id: format!("demo.cmd{i}"),
            title: format!("Demo Command {i}"),
            subtitle: Some(format!("demo subtitle {i}")),
            icon: String::new().into(),
            provider_id: "demo".into(),
            score: 0.0,
            keywords: vec![],
            category: Category::Application,
            actions: vec![],
            target: None,
        })
        .collect();

    let actions = vec![
        launcher_ui::ActionItem {
            action_id: "copy".into(),
            title: "Copy result".into(),
            enabled: true,
            reason: "".into(),
            shortcut: "Ctrl+Shift+C".into(),
            is_primary: true,
        },
        launcher_ui::ActionItem {
            action_id: "paste".into(),
            title: "Paste at cursor".into(),
            enabled: true,
            reason: "".into(),
            shortcut: "".into(),
            is_primary: false,
        },
        launcher_ui::ActionItem {
            action_id: "exec".into(),
            title: "Execute command".into(),
            enabled: false,
            reason: "Requires shell.execute".into(),
            shortcut: "".into(),
            is_primary: false,
        },
    ];

    let wf_items = vec![
        launcher_ui::WorkflowItem {
            step_id: "s1".into(),
            symbol: "✓".into(),
            title: "Scan files".into(),
            state_text: "done".into(),
            error: "".into(),
            current: false,
            execution_id: "e-101".into(),
            failure_class: "".into(),
        },
        launcher_ui::WorkflowItem {
            step_id: "s2".into(),
            symbol: "⏸".into(),
            title: "Move files".into(),
            state_text: "waiting for your confirmation".into(),
            error: "".into(),
            current: true,
            execution_id: "".into(),
            failure_class: "".into(),
        },
        launcher_ui::WorkflowItem {
            step_id: "s3".into(),
            symbol: "○".into(),
            title: "Generate report".into(),
            state_text: "pending".into(),
            error: "".into(),
            current: false,
            execution_id: "".into(),
            failure_class: "".into(),
        },
    ];

    (results, actions, wf_items)
}

/// Find the Launcher window handle by title (same mechanism as the
/// foreground module in main.rs).
#[cfg(windows)]
pub fn find_launcher_hwnd() -> Option<isize> {
    use windows::Win32::UI::WindowsAndMessaging::FindWindowW;
    unsafe {
        FindWindowW(None, &windows::core::HSTRING::from("Launcher"))
            .ok()
            .map(|h| h.0 as isize)
    }
}

#[cfg(windows)]
pub fn capture_client_to_bmp(hwnd: isize) -> Option<Vec<u8>> {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
        ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    };
    // PW_RENDERFULLCONTENT (Win 8.1+): capture the window's own rendering
    // even when it is not the foreground window — BitBlt-from-GetDC reads
    // the SCREEN and returns black for any occluded/background window,
    // which silently produced all-black baselines in headless capture runs.
    // windows@0.58 does not export PrintWindow; declare the user32 FFI here.
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
        // client-only content so the frame/decoration never leaks in
        let _ = PrintWindow(hwnd.0 as isize, hdc_mem.0 as isize, PW_RENDERFULLCONTENT);

        let mut bmi = BITMAPINFO::default();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = w;
        bmi.bmiHeader.biHeight = h; // bottom-up (canonical BMP; every consumer handles it)
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB.0;

        let mut pixels = vec![0u8; (w * h * 4) as usize];
        let got = GetDIBits(
            hdc_mem,
            hbmp,
            0,
            h as u32,
            Some(pixels.as_mut_ptr() as *mut _),
            &mut bmi,
            DIB_RGB_COLORS,
        );
        SelectObject(hdc_mem, old);
        let _ = DeleteObject(hbmp);
        let _ = DeleteDC(hdc_mem);
        ReleaseDC(hwnd, hdc_window);
        if got == 0 {
            return None;
        }

        // BMP file: header + info + BGRA (alpha byte used as padding = 0)
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
        bmp.extend_from_slice(&(0u32).to_le_bytes()); // BI_RGB
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
pub fn capture_client_to_bmp(_hwnd: isize) -> Option<Vec<u8>> {
    None
}

/// Populate the AppWindow with the deterministic demo dataset.
pub fn push_demo_data(ui: &launcher_ui::AppWindow) {
    let (results, actions, wf_items) = demo_data();
    let items: Vec<launcher_ui::ResultItem> = results
        .iter()
        .map(|c| launcher_ui::ResultItem {
            command_id: c.id.clone().into(),
            title: c.title.clone().into(),
            subtitle: c.subtitle.clone().unwrap_or_default().into(),
            icon: String::new().into(),
            score: String::new().into(),
            icon_data: slint::Image::default(),
        })
        .collect();
    ui.set_results(slint::ModelRc::new(std::rc::Rc::new(
        slint::VecModel::from(items),
    )));
    ui.set_actions(slint::ModelRc::new(std::rc::Rc::new(
        slint::VecModel::from(actions),
    )));
    ui.set_wf_items(slint::ModelRc::new(std::rc::Rc::new(
        slint::VecModel::from(wf_items),
    )));
    ui.set_wf_title("Demo Workflow".into());
    ui.set_wf_status("⏸ Paused".into());
    ui.set_wf_visible(true);
    ui.set_context_hint("📁 demo-context".into());
    ui.set_selected_index(0);
}

/// Snapshot entry point: returns Some(path) when the env var requested it.
pub fn snapshot_requested() -> Option<PathBuf> {
    std::env::var("LAUNCHER_SNAPSHOT")
        .ok()
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}
