use super::*;

impl CmdContext for Host {
    fn cbuf(&mut self) -> &mut Cbuf {
        if self.input_target.is_some() { &mut self.guest_cbuf } else { &mut self.cbuf }
    }
    fn cvars(&mut self) -> &mut Cvars {
        &mut self.server.core.cvars
    }
    fn commands(&mut self) -> &mut Commands<Self> {
        &mut self.cmds
    }
    fn print(&mut self, text: &str) {
        self.con.print(text);
    }
}

impl InputHost for Host {
    fn input(&mut self) -> &mut Input {
        match self.input_target {
            Some(k) if k < self.guests.len() => &mut self.guests[k].input,
            _ => &mut self.input,
        }
    }
}
