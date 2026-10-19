use gridcraft_engine::Session;
use serde_json::json;

#[test]
fn share_is_a_query_and_does_not_open_dialogs_or_mark_work_saved() {
    let mut s = Session::new();
    assert!(s.run("file.shareGridViewer", json!({})).is_err());
    s.new_workbook();
    s.run("cell.set", json!({"cell":"A1", "input":"42"})).unwrap();
    let undo = s.active().unwrap().undo.len();
    let result = s.run("file.shareGridViewer", json!({})).unwrap();
    assert!(result["url"].as_str().unwrap().contains("/grid#xlsx="));
    assert!(s.active().unwrap().is_dirty());
    assert_eq!(s.active().unwrap().undo.len(), undo);
    assert!(s.ui_requests.is_empty());
}

#[test]
fn oversized_workbook_requests_protected_sharing_without_uploading() {
    let mut s = Session::new();
    s.new_workbook();
    // Unique pseudo-random strings resist ZIP compression; no large fixture needed.
    let mut state = 17_u64;
    let rows: Vec<Vec<String>> = (0..2000)
        .map(|_| {
            let value: String = (0..80)
                .map(|_| {
                    state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                    char::from(b'a' + ((state >> 32) % 26) as u8)
                })
                .collect();
            vec![value]
        })
        .collect();
    s.run("range.setValues", json!({"range":"A1:A2000", "values":rows})).unwrap();
    let undo = s.active().unwrap().undo.len();
    let result = s.run("file.shareGridViewer", json!({})).unwrap();
    assert_eq!(result["requiresUpload"], true);
    assert!(result["bytes"].as_u64().unwrap() > 48 * 1024);
    assert!(result.get("url").is_none());
    assert!(result.get("base64").is_none());
    assert!(s.ui_requests.is_empty());
    assert_eq!(s.active().unwrap().undo.len(), undo);
    assert!(s.active().unwrap().is_dirty());
}
