//! CLI 层骨架。

/// CLI 命令类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Start,
    Stop,
    Restart,
    Status,
    Code,
    Model,
    Preset,
    Install,
    Activate,
    Ui,
    Unknown,
}

/// 解析首个命令参数。
pub fn parse_command(input: Option<&str>) -> Command {
    match input.unwrap_or_default() {
        "start" => Command::Start,
        "stop" => Command::Stop,
        "restart" => Command::Restart,
        "status" => Command::Status,
        "code" => Command::Code,
        "model" => Command::Model,
        "preset" => Command::Preset,
        "install" => Command::Install,
        "activate" | "env" => Command::Activate,
        "ui" => Command::Ui,
        _ => Command::Unknown,
    }
}
