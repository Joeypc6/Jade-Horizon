// GameRule.getDescriptionId: gamerule.<namespace>.<path>, in the jar's language files.
fn jade_rule_name_key(r: &Rule) -> core::ops::ControlFlow<String> {
    if r.name == gamerules::hz_jade_rule::NAME {
        core::ops::ControlFlow::Break("gamerule.jade.max_position_deviation".to_owned())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
