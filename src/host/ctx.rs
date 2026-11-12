use super::*;

impl CmdContext for Host {
    fn cbuf(&mut self) -> &mut Cbuf {
        &mut self.cbuf
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
        &mut self.input
    }
}
