use std::{io::Write, time::Duration};

use crossterm::{
    event::{poll, read, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};

use crate::{
    completion::{Completer, Completion, CompletionCtx, DefaultCompleter},
    cursor::{Cursor, DefaultCursor},
    history::{DefaultHistory, History},
    menu::{DefaultMenu, Menu},
    painter::Painter,
    prompt::Prompt,
};

#[derive(Builder)]
#[builder(pattern = "owned")]
#[builder(setter(prefix = "with"))]
pub struct Line {
    #[builder(default = "Box::new(DefaultMenu::new())")]
    #[builder(setter(custom))]
    menu: Box<dyn Menu<MenuItem = String>>,

    #[builder(default = "Box::new(DefaultCompleter::new(vec![]))")]
    #[builder(setter(custom))]
    completer: Box<dyn Completer>,

    #[builder(default = "Box::new(DefaultHistory::new())")]
    #[builder(setter(custom))]
    history: Box<dyn History<HistoryItem = String>>,

    #[builder(default = "Box::new(DefaultCursor::default())")]
    #[builder(setter(custom))]
    cursor: Box<dyn Cursor>,

    // ignored fields
    #[builder(setter(skip))]
    buf: Vec<u8>,
    #[builder(setter(skip))]
    ind: i32,
    // TODO this is temp, find better way to store prefix of current word
    #[builder(setter(skip))]
    current_word: String,
}

impl Default for Line {
    fn default() -> Self {
        LineBuilder::default().build().unwrap()
    }
}

// TODO none of the builder stuff is being autogenerated rn :()
impl LineBuilder {
    pub fn with_menu(mut self, menu: impl Menu<MenuItem = String> + 'static) -> Self {
        self.menu = Some(Box::new(menu));
        self
    }
    pub fn with_completer(mut self, completer: impl Completer + 'static) -> Self {
        self.completer = Some(Box::new(completer));
        self
    }
    pub fn with_history(mut self, history: impl History<HistoryItem = String> + 'static) -> Self {
        self.history = Some(Box::new(history));
        self
    }
    pub fn with_cursor(mut self, cursor: impl Cursor + 'static) -> Self {
        self.cursor = Some(Box::new(cursor));
        self
    }
}

impl Line {
    pub fn read_line<T: Prompt + ?Sized>(&mut self, prompt: impl AsRef<T>) -> String {
        // get line

        self.read_events(prompt).unwrap()
    }

    fn read_events<T: Prompt + ?Sized>(
        &mut self,
        prompt: impl AsRef<T>,
    ) -> crossterm::Result<String> {
        let mut painter = Painter::new();
        painter.init().unwrap();

        // TODO dumping history index here for now
        let mut history_ind: i32 = -1;

        enable_raw_mode()?;

        painter
            .paint(&prompt, &self.menu, "", self.ind as usize, &self.cursor)
            .unwrap();

        loop {
            if poll(Duration::from_millis(1000))? {
                let event = read()?;

                // handle menu events
                if self.menu.is_active() {
                    match event {
                        Event::Key(KeyEvent {
                            code: KeyCode::Enter,
                            modifiers: KeyModifiers::NONE,
                            ..
                        }) => {
                            let accepted = self.menu.accept().cloned();
                            if let Some(accepted) = accepted {
                                self.accept_completion(&accepted);
                            }
                        },
                        Event::Key(KeyEvent {
                            code: KeyCode::Esc,
                            modifiers: KeyModifiers::NONE,
                            ..
                        }) => {
                            self.menu.disactivate();
                        },
                        Event::Key(KeyEvent {
                            code: KeyCode::Tab,
                            modifiers: KeyModifiers::SHIFT,
                            ..
                        })
                        | Event::Key(KeyEvent {
                            code: KeyCode::Up,
                            modifiers: KeyModifiers::NONE,
                            ..
                        }) => {
                            self.menu.previous();
                        },
                        Event::Key(KeyEvent {
                            code: KeyCode::Tab,
                            modifiers: KeyModifiers::NONE,
                            ..
                        })
                        | Event::Key(KeyEvent {
                            code: KeyCode::Down,
                            modifiers: KeyModifiers::NONE,
                            ..
                        }) => {
                            self.menu.next();
                        },
                        _ => {},
                    }
                } else {
                    match event {
                        Event::Key(KeyEvent {
                            code: KeyCode::Enter,
                            modifiers: KeyModifiers::NONE,
                            ..
                        }) => {
                            painter.newline()?;
                            break;
                        },
                        Event::Key(KeyEvent {
                            code: KeyCode::Tab,
                            modifiers: KeyModifiers::NONE,
                            ..
                        }) => {
                            let res = std::str::from_utf8(self.buf.as_slice())
                                .unwrap()
                                .to_string();

                            // TODO IFS
                            let args = res.as_str()[..self.ind as usize].split(' ');
                            self.current_word = args.clone().last().unwrap_or("").to_string();

                            let ctx = CompletionCtx {
                                arg_num: args.count(),
                            };
                            let completions = self.completer.complete(&self.current_word, ctx);
                            let owned = completions
                                .iter()
                                .map(|x| x.to_string())
                                .take(10) // TODO make this config
                                .collect::<Vec<_>>();

                            // if completions only has one entry, automatically select it
                            if owned.len() == 1 {
                                self.accept_completion(owned.get(0).unwrap());
                            } else {
                                self.menu.set_items(owned);
                                self.menu.activate();
                            }
                        },
                        Event::Key(KeyEvent {
                            code: KeyCode::Left,
                            modifiers: KeyModifiers::NONE,
                            ..
                        }) => {
                            self.ind = (self.ind - 1).max(0);
                        },
                        Event::Key(KeyEvent {
                            code: KeyCode::Right,
                            modifiers: KeyModifiers::NONE,
                            ..
                        }) => {
                            self.ind = (self.ind + 1).min(self.buf.len() as i32);
                        },
                        Event::Key(KeyEvent {
                            code: KeyCode::Backspace,
                            modifiers: KeyModifiers::NONE,
                            ..
                        }) => {
                            if !self.buf.is_empty() {
                                self.ind = (self.ind - 1).max(0);
                                self.buf.remove(self.ind as usize);
                            }
                        },
                        Event::Key(KeyEvent {
                            code: KeyCode::Down,
                            modifiers: KeyModifiers::NONE,
                            ..
                        }) => {
                            history_ind = (history_ind - 1).max(0);
                            if let Some(history_item) = self.history.get(history_ind as usize) {
                                self.buf.clear();
                                let mut history_item =
                                    history_item.chars().map(|x| x as u8).collect::<Vec<_>>();
                                self.buf.append(&mut history_item);
                                self.ind = self.buf.len() as i32;
                            }
                        },
                        Event::Key(KeyEvent {
                            code: KeyCode::Up,
                            modifiers: KeyModifiers::NONE,
                            ..
                        }) => {
                            history_ind = if self.history.len() == 0 {
                                0
                            } else {
                                (history_ind + 1).min(self.history.len() as i32 - 1)
                            };
                            if let Some(history_item) = self.history.get(history_ind as usize) {
                                self.buf.clear();
                                let mut history_item =
                                    history_item.chars().map(|x| x as u8).collect::<Vec<_>>();
                                self.buf.append(&mut history_item);
                                self.ind = self.buf.len() as i32;
                            }
                        },
                        Event::Key(KeyEvent {
                            code: KeyCode::Char(c),
                            ..
                        }) => {
                            self.buf.insert(self.ind as usize, c as u8);
                            self.ind = (self.ind + 1).min(self.buf.len() as i32);
                        },
                        _ => {},
                    }
                }

                let res = std::str::from_utf8(self.buf.as_slice())
                    .unwrap()
                    .to_string();

                painter
                    .paint(&prompt, &self.menu, &res, self.ind as usize, &self.cursor)
                    .unwrap();
            }
        }

        disable_raw_mode()?;

        let res = std::str::from_utf8(self.buf.as_slice())
            .unwrap()
            .to_string();
        self.history.add(res.clone());
        Ok(res)
    }

    // replace word at cursor with accepted word (used in automcompletion)
    fn accept_completion(&mut self, accepted: &str) {
        // TODO this code is dumb
        // first remove current word
        self.buf.drain(
            (self.ind as usize).saturating_sub(self.current_word.len())..(self.ind as usize),
        );
        self.ind = (self.ind as usize).saturating_sub(self.current_word.len()) as i32;

        // then replace with the completion word
        accepted.clone().chars().for_each(|c| {
            // TODO find way to insert multiple items in one operation
            self.buf.insert(self.ind as usize, c as u8);
            self.ind = (self.ind + 1).min(self.buf.len() as i32);
        });
    }
}
