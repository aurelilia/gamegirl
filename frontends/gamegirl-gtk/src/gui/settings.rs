use adw::{prelude::*, subclass::prelude::*};
use gamegirl::{CgbMode, common::common::audio::AudioSampler, frontend::filter::Blend};
use gtk::{
    gio,
    glib::{self, clone},
};

use super::window::GameGirlWindow;
use crate::config::TextureFilter;

macro_rules! switch {
    ($window:ident, $prop:expr, $item:expr) => {{
        ($prop).set_active(*($item));
        ($prop).connect_active_notify(clone!(
            #[weak]
            $window,
            move |s| {
                *($item) = s.is_active();
            }
        ));
    }};
    ($window:ident, $prop:expr, $item:expr, $callback:expr) => {{
        ($prop).set_active(*($item));
        ($prop).connect_active_notify(clone!(
            #[weak]
            $window,
            move |s| {
                let e = s.is_active();
                *($item) = e;
                ($callback)(e);
            }
        ));
    }};
}
macro_rules! spin {
    ($window:ident, $prop:expr, $item:expr) => {{
        ($prop).set_value(*($item) as f64);
        ($prop).connect_activate(clone!(
            #[weak]
            $window,
            move |s| {
                *($item) = s.value() as usize;
            }
        ));
    }};
}
macro_rules! spin_percent {
    ($window:ident, $prop:expr, $item:expr) => {{
        ($prop).set_value(*($item) as f64 * 100.0);
        ($prop).connect_activate(clone!(
            #[weak]
            $window,
            move |s| {
                *($item) = s.value() as f32 * 100.0;
            }
        ));
    }};
}
macro_rules! combo {
    ($window:ident, $prop:expr, $item:expr, $list:expr) => {{
        let entries: &'static [_] = $list;
        let current = entries.iter().position(|x| x == ($item)).unwrap();
        ($prop).set_selected(current as u32);
        ($prop).connect_activate(clone!(
            #[weak]
            $window,
            move |s| {
                *($item) = entries[s.selected() as usize];
            }
        ));
    }};
    ($window:ident, $prop:expr, $item:expr, $list:expr, $callback:expr) => {{
        let entries: &'static [_] = $list;
        let current = entries.iter().position(|x| x == ($item)).unwrap();
        ($prop).set_selected(current as u32);
        ($prop).connect_activate(clone!(
            #[weak]
            $window,
            move |s| {
                let e = entries[s.selected() as usize];
                *($item) = e;
                ($callback)(e);
            }
        ));
    }};
}

glib::wrapper! {
    pub struct SettingsWindow(ObjectSubclass<imp::SettingsWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget,
        gtk::Native, gtk::Root, gtk::ShortcutManager, gio::ActionMap, gio::ActionGroup;
}

impl SettingsWindow {
    pub fn new<P: IsA<gtk::Application>>(app: &P, w: GameGirlWindow) -> Self {
        let this: Self = glib::Object::builder().property("application", app).build();
        let imp = this.imp();

        // Video
        combo!(
            w,
            imp.texture_filter,
            &mut w.imp().state.borrow_mut().options.texture_filter,
            &[
                TextureFilter::Nearest,
                TextureFilter::Linear,
                TextureFilter::Trilinear
            ],
            |filt| w.canvas().set_filter(filt)
        );
        combo!(
            w,
            imp.blend_filter,
            &mut w.imp().state.borrow_mut().options.blend_filter,
            &[Blend::None, Blend::Soften, Blend::Accumulate],
            |filt| w.canvas().set_blend_filter(filt)
        );
        // TODO frame skip
        // TODO pixel perfect scaling
        switch!(
            w,
            imp.preserve_aspect_ratio,
            &mut w.imp().state.borrow_mut().options.preserve_aspect_ratio,
            |p| w.set_preserve_aspect_ratio(p)
        );

        // Audio
        spin_percent!(
            w,
            imp.volume,
            &mut w.imp().state.borrow_mut().options.sys.volume
        );
        spin_percent!(
            w,
            imp.volume_ff,
            &mut w.imp().state.borrow_mut().options.sys.volume_ff
        );
        combo!(
            w,
            imp.output_sr,
            &mut w.imp().state.borrow_mut().options.sys.sample_rate,
            &[22_050, 44_100, 48_000, 96_000, 192_000]
        );
        combo!(
            w,
            imp.resample_alg,
            &mut w.imp().state.borrow_mut().options.sys.resampler,
            &[
                AudioSampler::Nearest,
                AudioSampler::Linear,
                AudioSampler::Cubic,
                AudioSampler::SincLinear { len: 128 },
                AudioSampler::SincCubic { len: 128 }
            ]
        );

        // Emulation
        switch!(
            w,
            imp.ggc_color_correction,
            &mut w.imp().state.borrow_mut().options.sys.cgb_colour_correction
        );
        combo!(
            w,
            imp.gg_mode_pref,
            &mut w.imp().state.borrow_mut().options.sys.mode,
            &[CgbMode::Always, CgbMode::Prefer, CgbMode::Never]
        );
        switch!(
            w,
            imp.gga_cpu_opt,
            &mut w.imp().state.borrow_mut().options.sys.cached_interpreter
        );
        switch!(
            w,
            imp.gga_threaded,
            &mut w.imp().state.borrow_mut().options.sys.threaded_ppu
        );

        // Features
        switch!(
            w,
            imp.run_on_rom_load,
            &mut w.imp().state.borrow_mut().options.sys.run_on_open
        );
        switch!(
            w,
            imp.skip_splash_screen,
            &mut w.imp().state.borrow_mut().options.sys.skip_bootrom
        );
        spin!(
            w,
            imp.fast_forward_speed_hold,
            &mut w.imp().state.borrow_mut().options.rewind.ff_hold_speed
        );
        spin!(
            w,
            imp.fast_forward_speed_hold,
            &mut w.imp().state.borrow_mut().options.rewind.ff_toggle_speed
        );

        this
    }
}

mod imp {
    use gtk::{glib, prelude::GtkWindowExt, subclass::prelude::*};

    #[derive(Default, Debug, gtk::CompositeTemplate)]
    #[template(resource = "/eu/catin/gamegirl/settings.ui")]
    pub struct SettingsWindow {
        // Video
        #[template_child]
        pub texture_filter: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub blend_filter: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub frame_skip: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub scale_pixel_perfect: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub preserve_aspect_ratio: TemplateChild<adw::SwitchRow>,

        // Audio
        #[template_child]
        pub volume: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub volume_ff: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub output_sr: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub resample_alg: TemplateChild<adw::ComboRow>,

        // Emulation
        #[template_child]
        pub ggc_color_correction: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub gg_mode_pref: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub gga_cpu_opt: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub gga_threaded: TemplateChild<adw::SwitchRow>,

        // Features
        #[template_child]
        pub run_on_rom_load: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub skip_splash_screen: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub fast_forward_speed_hold: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub fast_forward_speed_toggle: TemplateChild<adw::SpinRow>,
    }

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
