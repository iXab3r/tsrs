// EyeAuras fork: unregistered source-text slots are reused, so a host that frees its files never fills the table.

use tsrs_ast::{NO_SOURCE_TEXT, register_source_text, source_text_slots, unregister_source_text};

#[test]
fn unregistered_slots_are_reused_and_the_table_never_fills() {
    let a = register_source_text("let a = 1;");
    unregister_source_text(a);
    let b = register_source_text("let b = 2;");
    assert_eq!(a, b, "a freed slot is handed out again");
    unregister_source_text(b);
    unregister_source_text(b); // ignored: the slot is already free
    let before = source_text_slots();
    assert_eq!(before.free, 1);

    // Three times the table size of register/unregister pairs, as a host that builds and frees programs forever.
    for _ in 0..3 * before.capacity {
        let i = register_source_text("export const x = 1;");
        assert_ne!(i, NO_SOURCE_TEXT, "table exhausted");
        unregister_source_text(i);
    }
    let after = source_text_slots();
    assert_eq!(after.high_water, before.high_water, "no new slots were needed");
    assert_eq!(after.free, 1);
}
