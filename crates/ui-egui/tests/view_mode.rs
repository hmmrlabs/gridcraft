//! Viewing permits navigation but never workbook mutation through UI input.
use egui::{Event, Modifiers, PointerButton, vec2};
use gridcraft_engine::{Session, core::CellRef};
use gridcraft_ui_egui::{SheetApp, grid::Geo};
use serde_json::json;

#[test]
fn viewing_blocks_commands_typing_paste_double_click_and_resize() {
    let mut session = Session::new();
    session.new_workbook();
    session.execute("cell.set", json!({"cell":"A1","input":"42"})).unwrap();
    let mut app = SheetApp::new(session, Default::default());
    app.read_only = true;
    let mut h = egui_kittest::Harness::builder().with_size(vec2(1000.0, 700.0)).build_ui_state(
        |ui, app: &mut SheetApp| {
            app.logic(ui.ctx());
            app.view_ui(ui, true);
        },
        app,
    );
    h.run_steps(4);
    let before = serde_json::to_value(&h.state().session.active().unwrap().wb).unwrap();
    let sheet = h.state().session.active().unwrap().wb.active().unwrap();
    let geo = Geo::new(sheet, h.state().grid.rect.unwrap(), h.state().view().scroll);
    let at = geo.cell_rect(sheet, CellRef::new(0, 0)).center();
    let edge = egui::pos2(geo.cell_rect(sheet, CellRef::new(0, 0)).right(), geo.cells.top() - 5.0);
    for pos in [at, at, edge] {
        for pressed in [true, false] {
            h.input_mut().events.extend([
                Event::PointerMoved(pos),
                Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE },
            ]);
            h.step();
        }
    }
    h.input_mut().events.extend([
        Event::PointerMoved(edge),
        Event::PointerButton { pos: edge, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE },
    ]);
    h.step();
    h.input_mut().events.push(Event::PointerMoved(edge + vec2(80.0, 0.0)));
    h.step();
    h.input_mut().events.push(Event::PointerButton {
        pos: edge + vec2(80.0, 0.0),
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    h.step();
    h.input_mut().events.extend([Event::Text("999".into()), Event::Paste("888".into())]);
    h.run_steps(2);
    h.state_mut().begin_edit(Some("777".into()), false);
    for id in ["cell.set", "edit.paste", "home.columnWidth", "sheet.rename", "object.delete", "edit.undo"] {
        assert!(h.state_mut().run(id, json!({})).is_err());
    }
    assert!(h.state().editor.is_none());
    assert_eq!(serde_json::to_value(&h.state().session.active().unwrap().wb).unwrap(), before);
    assert!(h.state_mut().run("sheet.activate", json!({"sheet":0})).is_ok());
}
