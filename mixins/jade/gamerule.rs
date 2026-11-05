// Jade.registerGameRules: GameRules.registerInteger("jade:max_position_deviation", MISC, 21, 0, 1000).
pub const NAME: &str = "jade:max_position_deviation";

pub static RULE: super::Rule = super::Rule {
    name: NAME,
    category: "misc",
    default: super::Value::Int(21),
    min: 0,
    max: 1000,
};
