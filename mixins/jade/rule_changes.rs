fn jade_rule_changes(screen: &RulesScreen) -> core::ops::ControlFlow<Vec<(&'static str, Value)>> {
    let jade: &'static Rule = &gamerules::hz_jade_rule::RULE;
    let changes = screen
        .values
        .available_rules()
        .chain(std::iter::once(jade))
        .filter_map(|r| {
            let value = screen.values.get(r.name)?;
            (Some(value) != screen.initial.get(r.name)).then_some((r.name, value))
        })
        .collect();
    core::ops::ControlFlow::Break(changes)
}
