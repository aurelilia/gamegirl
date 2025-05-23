use std::{
    fs::{self, OpenOptions},
    io::Write,
};

use adw::{Toast, subclass::prelude::ObjectSubclassIsExt};
use gamegirl::{GameCart, Storage};
use gtk::{Label, gio::prelude::FileExt, glib::Variant};

use super::window::GameGirlWindow;

impl GameGirlWindow {
    pub async fn open_file(&self) {
        let dialog = gtk::FileDialog::builder()
            .title("Open File")
            .accept_label("Open")
            .modal(true)
            .build();
        let file = dialog.open_future(Some(self)).await;
        match file
            .ok()
            .map(|f| f.path().and_then(|p| fs::read(&p).ok().map(|b| (p, b))))
        {
            // Got a ROM
            Some(Some((path, rom))) => {
                let title = format!("gamegirl - {}", path.file_stem().unwrap().display());
                let save = Storage::load(Some(path.clone()), "".into());

                match gamegirl::load_cart(
                    GameCart { rom, save },
                    &self.imp().state.borrow().options.sys,
                ) {
                    Ok(sys) => {
                        *self.core().lock().unwrap() = sys;
                        self.imp().state.borrow_mut().current_rom_path = Some(path);
                        self.toast(Toast::new("Loaded ROM!"));

                        let label = Label::builder().label(title).css_classes(["title"]).build();
                        self.imp().header.set_title_widget(Some(&label));
                    }
                    Err(err) => {
                        self.toast(Toast::new(&format!("Failed to load ROM: {}", err)));
                    }
                }
            }
            // Failed getting path or reading out file
            Some(None) => {
                self.toast(Toast::new("Failed to load ROM!"));
            }
            // User aborted
            None => (),
        }
    }

    pub fn save_game(&self) {
        let core = self.core();
        let save = core.lock().unwrap().make_save();
        if let Some(save) = save {
            Storage::save(self.imp().state.borrow().current_rom_path.clone(), save);
        }
    }

    pub async fn save_game_as(&self) {
        let core = self.core();
        let save = core.lock().unwrap().make_save();
        if let Some(save) = save {
            let dialog = gtk::FileDialog::builder()
                .title("Save Game")
                .accept_label("Save")
                .modal(true)
                .build();
            let file = dialog.open_future(Some(self)).await;
            match file.ok().map(|f| {
                f.path().and_then(|p| {
                    OpenOptions::new()
                        .write(true)
                        .create(true)
                        .open(&p)
                        .and_then(|mut f| f.write_all(&save.ram))
                        .ok()
                        .map(|_| p)
                })
            }) {
                // Valid path
                Some(Some(path)) => {
                    self.toast(Toast::new(&format!("Saved to {}", path.display())));
                }
                // Failed getting path or writing out file
                Some(None) => {
                    self.toast(Toast::new("Failed to save to file!"));
                }
                // User aborted
                None => (),
            }
        }
    }

    pub fn playpause(&self) {
        let core = self.core();
        let mut core = core.lock().unwrap();
        let c = core.c_mut();
        c.debugger.running = !c.debugger.running;
        if c.debugger.running {
            self.toast(Toast::new("Resuming"));
        } else {
            self.toast(Toast::new("Paused"));
        }
    }

    pub fn save_state(&self, slot: &Variant) {
        let slot = slot.get::<u32>().unwrap() as usize - 1;
        let save = self.core().lock().unwrap().save_state();
        let rev = &mut self.imp().state.borrow_mut().rewinder;
        rev.save_states[slot] = Some(save);
        self.toast(Toast::new("Saved state"));
    }

    pub fn load_state(&self, slot: &Variant) {
        let slot = slot.get::<u32>().unwrap() as usize - 1;
        let core = self.core();
        let mut core = core.lock().unwrap();
        let save = core.save_state();
        let rev = &mut self.imp().state.borrow_mut().rewinder;
        let to_load = if slot == 5 {
            &rev.before_last_ss_load
        } else {
            &rev.save_states[slot]
        };
        if let Some(to_load) = to_load {
            println!("loading state {slot}: size: {}", to_load.len());
            core.load_state(&to_load);
            rev.before_last_ss_load = Some(save);
            self.toast(Toast::new("Loaded state"));
        } else {
            self.toast(Toast::new("Save to this state first!"));
        }
    }

    pub async fn save_state_as(&self) {
        let core = self.core();
        let save = core.lock().unwrap().save_state();
        let dialog = gtk::FileDialog::builder()
            .title("Save State")
            .accept_label("Save")
            .modal(true)
            .build();
        let file = dialog.save_future(Some(self)).await;
        match file.ok().map(|f| {
            f.path().and_then(|p| {
                OpenOptions::new()
                    .write(true)
                    .create(true)
                    .open(&p)
                    .and_then(|mut f| f.write_all(&save))
                    .ok()
                    .map(|_| p)
            })
        }) {
            // Valid path
            Some(Some(path)) => {
                self.toast(Toast::new(&format!("Saved to {}", path.display())));
            }
            // Failed getting path or writing out file
            Some(None) => {
                self.toast(Toast::new("Failed to save to file!"));
            }
            // User aborted
            None => (),
        }
    }

    pub async fn load_state_as(&self) {
        let core = self.core();
        let dialog = gtk::FileDialog::builder()
            .title("Load State")
            .accept_label("Load")
            .modal(true)
            .build();
        let file = dialog.open_future(Some(self)).await;
        match file
            .ok()
            .map(|f| f.path().and_then(|p| fs::read(&p).ok().map(|b| (p, b))))
        {
            // Valid path and file
            Some(Some((path, save))) => {
                core.lock().unwrap().load_state(&save);
                self.toast(Toast::new(&format!("Loaded state from {}", path.display())));
            }
            // Failed getting path or reading out file
            Some(None) => {
                self.toast(Toast::new("Failed to load file!"));
            }
            // User aborted
            None => (),
        }
    }
}
