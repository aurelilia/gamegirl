use gtk::{gio, glib, prelude::*};

glib::wrapper! {
    pub struct SettingsWindow(ObjectSubclass<imp::SettingsWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget,
        gtk::Native, gtk::Root, gtk::ShortcutManager, gio::ActionMap, gio::ActionGroup;
}

impl SettingsWindow {
    pub fn new<P: IsA<gtk::Application>>(app: &P) -> Self {
        glib::Object::builder().property("application", app).build()
    }
}

mod imp {
    use gtk::{glib, prelude::GtkWindowExt, subclass::prelude::*};

    #[derive(Default, Debug, gtk::CompositeTemplate)]
    #[template(resource = "/eu/catin/gamegirl/settings.ui")]
    pub struct SettingsWindow {}

    #[glib::object_subclass]
    impl ObjectSubclass for SettingsWindow {
        const NAME: &'static str = "SettingsWindow";
        type Type = super::SettingsWindow;
        type ParentType = gtk::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.install_action("win.exit", None, |win, _action_name, _action_target| {
                win.destroy();
            });
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SettingsWindow {}
    impl WidgetImpl for SettingsWindow {}
    impl WindowImpl for SettingsWindow {}
    impl ApplicationWindowImpl for SettingsWindow {}
}
