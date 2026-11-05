// IntegerRuleEntry's box, as RulesScreen::build makes the others.
fn jade_rule_number(screen: &mut RulesScreen, name: &str) {
    let rule = &gamerules::hz_jade_rule::RULE;
    if name != rule.name || screen.numbers.iter().any(|(n, _)| *n == rule.name) {
        return;
    }
    let mut b = EditBox::new(0.0, 0.0, 44.0, 20.0);
    b.focused = false;
    b.set_value(&screen.values.int(rule.name).to_string());
    b.text_color = VALID;
    screen.numbers.push((rule.name, b));
}
