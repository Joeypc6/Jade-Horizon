// GameRuleCommand: gamerule <rule> [<value>].
fn jade_rule_command_tree(value: Dispatcher) -> Dispatcher {
    let mut d = value;
    let rule = &horizon_client_core::gamerules::hz_jade_rule::RULE;
    if let Some(gamerule) = d.find("gamerule") {
        let r = d.literal(gamerule, rule.name);
        d.executes(r, "gamerule");
        let v = d.argument(r, "value", ArgType::Integer { min: rule.min, max: rule.max });
        d.executes(v, "gamerule");
    }
    d
}
