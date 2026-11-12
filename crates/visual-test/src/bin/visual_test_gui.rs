use std::path::PathBuf;

fn main() {
    let report_dir = std::env::args().nth(1).map(PathBuf::from);
    visual_test::ui::run_desktop_app(report_dir);
}
