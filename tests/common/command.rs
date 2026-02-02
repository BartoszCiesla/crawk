#![allow(unused)]
use std::collections::HashMap;
use std::convert::Into;

pub(crate) struct TestArgs {
    opts: Vec<String>,
    cmd: Vec<String>,
    cmd_opts: Vec<String>,
    args: Vec<String>,
    env: HashMap<String, String>,
}

impl TestArgs {
    pub(crate) fn new() -> Self {
        Self {
            opts: vec![],
            cmd: vec![],
            cmd_opts: vec![],
            args: vec![],
            env: HashMap::new(),
        }
    }

    pub(crate) fn opt(mut self, arg: impl Into<String>) -> Self {
        self.opts.push(arg.into());
        self
    }

    pub(crate) fn opts(mut self, opts: Vec<impl Into<String>>) -> Self {
        self.opts.append(
            opts.into_iter()
                .map(Into::into)
                .collect::<Vec<_>>()
                .as_mut(),
        );
        self
    }

    pub(crate) fn cmd(mut self, arg: impl Into<String>) -> Self {
        self.cmd.push(arg.into());
        self
    }

    pub(crate) fn cmd_opt(mut self, arg: impl Into<String>) -> Self {
        self.cmd_opts.push(arg.into());
        self
    }

    pub(crate) fn cmd_opts(mut self, opts: Vec<impl Into<String>>) -> Self {
        self.cmd_opts.append(
            opts.into_iter()
                .map(Into::into)
                .collect::<Vec<_>>()
                .as_mut(),
        );
        self
    }

    pub(crate) fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub(crate) fn args(mut self, args: Vec<impl Into<String>>) -> Self {
        self.args.append(
            args.into_iter()
                .map(Into::into)
                .collect::<Vec<_>>()
                .as_mut(),
        );
        self
    }

    pub(crate) fn get_opts_and_args(&self) -> Vec<String> {
        let mut cmd = vec![];
        cmd.append(&mut self.opts.clone());
        cmd.append(&mut self.cmd.clone());
        cmd.append(&mut self.cmd_opts.clone());
        cmd.append(&mut self.args.clone());
        cmd
    }

    pub(crate) fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    pub(crate) fn get_env(&self) -> HashMap<String, String> {
        self.env.clone()
    }

    pub(crate) fn disable_backtrace(self) -> Self {
        self.env("RUST_BACKTRACE", "0")
    }
}
