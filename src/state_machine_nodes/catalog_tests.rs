use super::FormulaCatalog;

const CHATAIGNE_FORMULAS: &str = include_str!("../../builtin_formulas/chataigne.formulas.json");

#[test]
fn shipped_chataigne_formulas_do_not_seed_processor_templates() {
    let catalog = FormulaCatalog::from_builtin_package_source(CHATAIGNE_FORMULAS)
        .expect("shipped Chataigne formula package should decode");

    assert_eq!(catalog.processor_palette_entries().count(), 0);
}
