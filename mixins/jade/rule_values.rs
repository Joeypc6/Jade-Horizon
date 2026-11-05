// The reply this function makes to ServerboundClientCommandPacket's REQUEST_GAMERULE_VALUES, with Jade's rule too.
fn jade_rule_values(game: &crate::game::Game<'_>, host: &mut LanHost, conn: u32, packet: &S) -> core::ops::ControlFlow<()> {
    let S::ClientCommand(p) = packet else {
        return core::ops::ControlFlow::Continue(());
    };
    if p.action != 2 || !host.hub.guests.iter().any(|g| g.conn == conn && g.permission >= 2) {
        return core::ops::ControlFlow::Continue(());
    }
    let rules = &game.ui.rules;
    let mut entries: Vec<play::GameRuleEntry> = rules
        .available_rules()
        .map(|r| play::GameRuleEntry {
            name: format!("minecraft:{}", r.name),
            value: rules.get(r.name).unwrap_or(r.default).to_string(),
        })
        .collect();
    let jade = &horizon_client::gamerules::hz_jade_rule::RULE;
    entries.push(play::GameRuleEntry {
        name: jade.name.to_owned(),
        value: rules.get(jade.name).unwrap_or(jade.default).to_string(),
    });
    host.hub.send_to(conn, &C::GameRuleValues(play::GameRuleValues { entries }));
    core::ops::ControlFlow::Break(())
}
