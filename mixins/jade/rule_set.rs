// MinecraftServer.onGameRuleChanged names a rule by its own id, here jade:, not minecraft:jade:.
fn jade_rule_set(game: &mut crate::game::Game<'_>, rule: &gamerules::Rule, value: Value) -> core::ops::ControlFlow<()> {
    if rule.name != horizon_client::gamerules::hz_jade_rule::NAME {
        return core::ops::ControlFlow::Continue(());
    }
    game.ui.rules.set(rule.name, value);
    game.sync_rules();
    if game.options.dedicated.is_some()
        && let Value::Int(number) = value
    {
        super::dedicated::events::record_server(
            "gamerule",
            serde_json::json!({ "key": rule.name, "valueType": "integer", "value": number }),
        );
    }
    core::ops::ControlFlow::Break(())
}
