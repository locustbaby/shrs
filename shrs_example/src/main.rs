use std::default;

use shrs::{
    builtin::Builtins,
    find_executables_in_path,
    hooks::{Hooks, StartupHookCtx},
    line::{
        completion::DefaultCompleter, DefaultCursor, DefaultHighlighter, DefaultHistory,
        DefaultMenu, Line, LineBuilder, Prompt,
    },
    prompt::{hostname, top_pwd, username},
    Alias, Context, Env, Runtime, ShellConfig, ShellConfigBuilder,
};

struct MyPrompt;

impl Prompt for MyPrompt {
    fn prompt_left(&self) -> String {
        format!(" {} > ", top_pwd())
    }
}

fn main() {
    let mut env = Env::new();
    env.load();

    // configure line
    let completions: Vec<String> = find_executables_in_path(env.get("PATH").unwrap());
    let completer = DefaultCompleter::new(completions);
    let menu = DefaultMenu::new();
    let history = DefaultHistory::new();
    let cursor = DefaultCursor::default();
    let highlighter = DefaultHighlighter::default();

    let readline = LineBuilder::default()
        .with_cursor(cursor)
        .with_completer(completer)
        .with_menu(menu)
        .with_history(history)
        .with_highlighter(highlighter)
        .build()
        .unwrap();

    let prompt = MyPrompt;

    let alias = Alias::from_iter([
        ("l".into(), "ls".into()),
        ("c".into(), "cd".into()),
        ("g".into(), "git".into()),
        ("v".into(), "vim".into()),
        ("la".into(), "ls -a".into()),
    ]);

    let hooks = Hooks {
        startup: |_ctx: StartupHookCtx| {
            let welcome_str = format!(
                r#"
        __         
   ___ / /  _______
  (_-</ _ \/ __(_-<
 /___/_//_/_/ /___/
a rusty POSIX shell | build {}"#,
                env!("SHRS_VERSION")
            );

            println!("{}", welcome_str);
        },
        ..Default::default()
    };

    let myshell = ShellConfigBuilder::default()
        .with_hooks(hooks)
        .with_env(env)
        .with_alias(alias)
        .with_readline(readline)
        .with_prompt(prompt)
        .build()
        .unwrap();

    myshell.run();
}
