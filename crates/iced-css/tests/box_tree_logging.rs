use iced_css::{
    box_tree::{BoxTree, attribute_limits, capture_limits},
    capture::adapters,
};
use std::sync::Mutex;

struct Sink(Mutex<Vec<String>>);
static SINK: Sink = Sink(Mutex::new(Vec::new()));
impl log::Log for Sink {
    fn enabled(&self, m: &log::Metadata<'_>) -> bool {
        m.target() == "iced_css::stylesheet"
    }
    fn log(&self, r: &log::Record<'_>) {
        if self.enabled(r.metadata()) {
            self.0.lock().unwrap().push(r.args().to_string());
        }
    }
    fn flush(&self) {}
}

#[test]
fn a_box_tree_logs_what_its_sheets_drop() {
    log::set_logger(&SINK).unwrap();
    log::set_max_level(log::LevelFilter::Error);
    let root = adapters::column_from_vec::<(), iced::Theme, ()>(vec![], capture_limits())
        .unwrap()
        .style_id("page", attribute_limits())
        .unwrap()
        .erase();
    let tree = BoxTree::new(root, "#page { height:10px; }").unwrap();
    assert!(
        SINK.0.lock().unwrap().is_empty(),
        "a clean sheet logs nothing"
    );
    let next = tree
        .restyled(&[
            "#page { height:10px; }",
            "#page { paddng:16px; background-color:#f4f1eaa0z; }",
        ])
        .unwrap();
    assert_eq!(
        next.layout(100.0, None).unwrap()[0].unwrap().border.bottom,
        10.0
    );
    let logged = SINK.0.lock().unwrap().clone();
    assert_eq!(logged.len(), 2, "{logged:?}");
    // The second sheet is resource 2.
    assert!(
        logged.iter().all(|line| line.contains("ResourceId(2)")),
        "{logged:?}"
    );
}
