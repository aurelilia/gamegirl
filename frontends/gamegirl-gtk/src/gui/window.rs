use std::sync::{Arc, Mutex};

use adw::Toast;
use gamegirl::Core;
use gtk::{gio, glib, prelude::*, subclass::prelude::*};

use super::canvas::{self, GamegirlPaintable};

glib::wrapper! {
    pub struct GameGirlWindow(ObjectSubclass<imp::GameGirlWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget,
        gtk::Native, gtk::Root, gtk::ShortcutManager, gio::ActionMap, gio::ActionGroup;
}

impl GameGirlWindow {
    pub fn new<P: IsA<gtk::Application>>(app: &P) -> Self {
        let this: Self = glib::Object::builder().property("application", app).build();
        canvas::bind(&this.imp().game);
        this
    }

    pub fn toast(&self, toast: Toast) {
        self.imp().toast.add_toast(toast);
    }

    pub fn core(&self) -> Arc<Mutex<Box<dyn Core>>> {
        self.imp()
            .game
            .paintable()
            .unwrap()
            .downcast::<GamegirlPaintable>()
            .unwrap()
            .imp()
            .core
            .clone()
    }
}

mod imp {
    use std::cell::RefCell;

    use gtk::{
        glib::{self, VariantTy},
        prelude::GtkWindowExt,
        subclass::prelude::*,
    };

    use crate::{AppState, gui::settings::SettingsWindow};

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/eu/catin/gamegirl/main.ui")]
    pub struct GameGirlWindow {
        #[template_child]
        pub header: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub game: TemplateChild<gtk::Picture>,
        #[template_child]
        pub toast: TemplateChild<adw::ToastOverlay>,

        pub state: RefCell<AppState>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GameGirlWindow {
        const NAME: &'static str = "GameGirlWindow";
        type Type = super::GameGirlWindow;
        type ParentType = gtk::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.install_action_async(
                "win.open",
                None,
                |win, _action_name, _action_target| async move { win.open_file().await },
            );
            klass.install_action("win.save", None, |win, _action_name, _action_target| {
                win.save_game()
            });
            klass.install_action_async(
                "win.save_as",
                None,
                |win, _action_name, _action_target| async move { win.save_game_as().await },
            );
            klass.install_action(
                "win.save_state",
                Some(VariantTy::UINT32),
                |win, _action_name, action_target| win.save_state(action_target.unwrap()),
            );
            klass.install_action(
                "win.load_state",
                Some(VariantTy::UINT32),
                |win, _action_name, action_target| win.load_state(action_target.unwrap()),
            );
            klass.install_action_async(
                "win.save_state_as",
                None,
                |win, _action_name, _action_target| async move { win.save_state_as().await },
            );
            klass.install_action_async(
                "win.load_state_as",
                None,
                |win, _action_name, _action_target| async move { win.load_state_as().await },
            );
            klass.install_action("win.reset", None, |win, _action_name, _action_target| {
                win.core().lock().unwrap().reset();
            });
            klass.install_action(
                "win.playpause",
                None,
                |win, _action_name, _action_target| win.playpause(),
            );
            klass.install_action(
                "win.open_settings",
                None,
                |win, _action_name, _action_target| {
                    let win = SettingsWindow::new(&win.application().unwrap());
                    win.present();
                },
            );
            klass.install_action("win.exit", None, |win, _action_name, _action_target| {
                win.destroy();
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for GameGirlWindow {}
    impl WidgetImpl for GameGirlWindow {}
    impl WindowImpl for GameGirlWindow {}
    impl ApplicationWindowImpl for GameGirlWindow {}
}
