use crate::runtime::protocol::CommandSnapshot;
use crate::runtime::session::keyboard::InputDelivery;
#[derive(Default)]
pub(super) struct CommandInputHistory {
    interrupt_delivered: bool,
}
enum CommandTermination {
    Exited(i32),
    Interrupted,
}
impl CommandInputHistory {
    pub(super) const fn observe(&mut self, delivery: InputDelivery) {
        self.interrupt_delivered |= delivery.interrupted();
    }
    pub(super) const fn normalize(&self, snapshot: &mut CommandSnapshot) {
        let Some(reported_code) = snapshot.command.exit_code else {
            return;
        };
        snapshot.command.exit_code = Some(self.normalized_exit_code(reported_code));
    }
    pub(super) const fn normalized_exit_code(&self, reported_code: i32) -> i32 {
        CommandTermination::classify(reported_code, self.interrupt_delivered).exit_code()
    }
}
impl CommandTermination {
    const fn classify(reported_code: i32, interrupt_delivered: bool) -> Self {
        if interrupt_delivered && reported_code != 0_i32 {
            Self::Interrupted
        } else {
            Self::Exited(reported_code)
        }
    }
    const fn exit_code(self) -> i32 {
        match self {
            Self::Exited(code) => code,
            Self::Interrupted => 130_i32,
        }
    }
}
