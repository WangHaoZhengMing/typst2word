//! Native macOS System Menu Bar (`cx.set_menus`) and global keyboard shortcuts (`cx.bind_keys`).

use gpui_kit::{App, KeyBinding, Menu, MenuItem};

gpui_kit::actions!(
    visual_test,
    [
        QuitApp,
        RunActiveTarget,
        RunSelectedWithWord,
        RunCurrentSuiteWithWord,
        VerifyAllIr,
        ReloadReport,
        FocusCaseSearch,
        FindInEditor,
        PrevCase,
        NextCase,
        ZoomIn,
        ZoomOut,
        ZoomReset,
        ToggleLeftSidebar,
        ToggleRightInspector,
        ToggleBottomDrawer,
        ToggleAppearance,
        ShowDocTypst,
        ShowDocIr,
        ShowDocBodyXml,
        ShowDocSettingsXml,
        OpenDocxInWord,
        RevealCaseInFinder,
    ]
);

/// Register native macOS menu bar items and keyboard bindings so `visual-test-gui`
/// behaves like a first-class macOS developer application (Xcode / Preview / Instruments).
pub fn install_macos_menus_and_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-q", QuitApp, None),
        KeyBinding::new("cmd-r", RunActiveTarget, None),
        KeyBinding::new("cmd-shift-r", RunSelectedWithWord, None),
        KeyBinding::new("cmd-alt-r", RunCurrentSuiteWithWord, None),
        KeyBinding::new("cmd-shift-i", VerifyAllIr, None),
        KeyBinding::new("cmd-shift-l", ReloadReport, None),
        KeyBinding::new("cmd-k", FocusCaseSearch, None),
        KeyBinding::new("cmd-f", FindInEditor, None),
        KeyBinding::new("cmd-[", PrevCase, None),
        KeyBinding::new("cmd-]", NextCase, None),
        KeyBinding::new("alt-up", PrevCase, None),
        KeyBinding::new("alt-down", NextCase, None),
        KeyBinding::new("cmd-=", ZoomIn, None),
        KeyBinding::new("cmd-+", ZoomIn, None),
        KeyBinding::new("cmd--", ZoomOut, None),
        KeyBinding::new("cmd-0", ZoomReset, None),
        KeyBinding::new("cmd-alt-s", ToggleLeftSidebar, None),
        KeyBinding::new("cmd-alt-i", ToggleRightInspector, None),
        KeyBinding::new("cmd-alt-d", ToggleBottomDrawer, None),
        KeyBinding::new("cmd-shift-a", ToggleAppearance, None),
        KeyBinding::new("cmd-1", ShowDocTypst, None),
        KeyBinding::new("cmd-2", ShowDocIr, None),
        KeyBinding::new("cmd-3", ShowDocBodyXml, None),
        KeyBinding::new("cmd-4", ShowDocSettingsXml, None),
    ]);

    cx.on_action(|_: &QuitApp, cx| {
        cx.quit();
    });

    cx.set_menus(vec![
        Menu {
            name: "Visual Test".into(),
            items: vec![
                MenuItem::action("Reload Report from Disk", ReloadReport),
                MenuItem::separator(),
                MenuItem::action("Quit Visual Test", QuitApp),
            ],
            disabled: false,
        },
        Menu {
            name: "Run".into(),
            items: vec![
                MenuItem::action("Run Active Scheme", RunActiveTarget),
                MenuItem::action("Render Selected Case in Word", RunSelectedWithWord),
                MenuItem::action("Run Current .typ Suite in Word", RunCurrentSuiteWithWord),
                MenuItem::action("Verify All Cases (IR & OpenXML)", VerifyAllIr),
                MenuItem::separator(),
                MenuItem::action("Open output.docx in Microsoft Word", OpenDocxInWord),
                MenuItem::action("Reveal Case Folder in Finder", RevealCaseInFinder),
            ],
            disabled: false,
        },
        Menu {
            name: "View".into(),
            items: vec![
                MenuItem::action("Toggle Stdlib Navigator", ToggleLeftSidebar),
                MenuItem::action("Toggle Code Inspector", ToggleRightInspector),
                MenuItem::action("Toggle Telemetry Drawer", ToggleBottomDrawer),
                MenuItem::separator(),
                MenuItem::action("Zoom In", ZoomIn),
                MenuItem::action("Zoom Out", ZoomOut),
                MenuItem::action("Actual Size (100%)", ZoomReset),
                MenuItem::separator(),
                MenuItem::action("Toggle Light / Dark Appearance", ToggleAppearance),
            ],
            disabled: false,
        },
        Menu {
            name: "Inspector".into(),
            items: vec![
                MenuItem::action("Typst Source (.typ)", ShowDocTypst),
                MenuItem::action("Semantic IR (ir.json)", ShowDocIr),
                MenuItem::action("Word Body XML (document.xml)", ShowDocBodyXml),
                MenuItem::action("Word Settings XML (settings.xml)", ShowDocSettingsXml),
                MenuItem::separator(),
                MenuItem::action("Find in Code Inspector…", FindInEditor),
            ],
            disabled: false,
        },
        Menu {
            name: "Navigate".into(),
            items: vec![
                MenuItem::action("Search Cases…", FocusCaseSearch),
                MenuItem::separator(),
                MenuItem::action("Previous Case", PrevCase),
                MenuItem::action("Next Case", NextCase),
            ],
            disabled: false,
        },
    ]);
}
