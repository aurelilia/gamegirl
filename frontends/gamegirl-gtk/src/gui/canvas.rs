use adw::subclass::prelude::ObjectSubclassIsExt;
use gamegirl::frontend::filter::Blend;
use gtk::{
    gdk,
    glib::{self},
    gsk::ScalingFilter,
    prelude::*,
};

use crate::config::TextureFilter;

pub fn bind(pict: &gtk::Picture) -> GamegirlPaintable {
    assert!(
        pict.paintable().is_none(),
        "Canvas was bound more than once!"
    );
    let draw = GamegirlPaintable::new();
    pict.set_paintable(Some(&draw));
    pict.add_tick_callback(|pict, _| {
        pict.queue_draw();
        glib::ControlFlow::Continue
    });
    draw
}

glib::wrapper! {
    pub struct GamegirlPaintable(ObjectSubclass<imp::GamegirlPaintable>) @implements gdk::Paintable;
}

impl GamegirlPaintable {
    pub fn set_filter(&self, filter: TextureFilter) {
        let filter = match filter {
            TextureFilter::Nearest => ScalingFilter::Nearest,
            TextureFilter::Linear => ScalingFilter::Linear,
            TextureFilter::Trilinear => ScalingFilter::Trilinear,
        };
        self.imp().filter.set(filter);
    }

    pub fn set_blend_filter(&self, blend: Blend) {
        self.imp().blend.set(blend);
    }

    pub fn new() -> Self {
        glib::Object::new()
    }
}

mod imp {
    use std::{
        cell::{Cell, RefCell},
        mem,
        sync::{Arc, Mutex},
    };

    use gamegirl::{
        Core,
        frontend::{
            self,
            cpal::AudioStream,
            filter::{Blend, ScreenBuffer},
        },
    };
    use gtk::{
        gdk::{self, subclass::prelude::PaintableImpl},
        glib::{
            self,
            subclass::{object::ObjectImpl, types::ObjectSubclass},
        },
        graphene::{self, Rect},
        gsk::ScalingFilter,
        prelude::SnapshotExt,
    };

    pub struct GamegirlPaintable {
        pub core: Arc<Mutex<Box<dyn Core>>>,
        _audio_stream: AudioStream,
        last_texture: Mutex<Option<gdk::MemoryTexture>>,

        buffer: RefCell<ScreenBuffer>,
        pub(super) filter: Cell<ScalingFilter>,
        pub(super) blend: Cell<Blend>,
    }

    impl Default for GamegirlPaintable {
        fn default() -> Self {
            let core = Arc::new(Mutex::new(gamegirl::dummy_core()));
            let _audio_stream = frontend::cpal::setup(core.clone());
            Self {
                core,
                _audio_stream,
                last_texture: Mutex::new(None),

                buffer: RefCell::default(),
                filter: Cell::new(ScalingFilter::Nearest),
                blend: Cell::new(Blend::None),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GamegirlPaintable {
        const NAME: &'static str = "GamegirlPaintable";
        type Type = super::GamegirlPaintable;
        type Interfaces = (gdk::Paintable,);
    }

    impl ObjectImpl for GamegirlPaintable {}

    impl PaintableImpl for GamegirlPaintable {
        fn flags(&self) -> gdk::PaintableFlags {
            gdk::PaintableFlags::CONTENTS
        }

        fn intrinsic_width(&self) -> i32 {
            self.core.lock().unwrap().screen_size()[0] as i32
        }

        fn intrinsic_height(&self) -> i32 {
            self.core.lock().unwrap().screen_size()[1] as i32
        }

        fn snapshot(&self, snapshot: &gdk::Snapshot, width: f64, height: f64) {
            let mut core = self.core.lock().unwrap();
            let mut last_texture = self.last_texture.lock().unwrap();
            let maybe_frame = core.c_mut().video_buffer.pop();
            if let Some(frame) = maybe_frame {
                let frame = self.buffer.borrow_mut().next_frame(frame, self.blend.get());
                let size = core.screen_size();
                let byte_vec = unsafe {
                    let mut byte_vec: Vec<u8> = mem::transmute(frame);
                    byte_vec.set_len(byte_vec.len() * 4);
                    byte_vec
                };
                let bytes = glib::Bytes::from_owned(byte_vec);
                *last_texture = Some(gdk::MemoryTexture::new(
                    size[0] as i32,
                    size[1] as i32,
                    gdk::MemoryFormat::R8g8b8a8,
                    &bytes,
                    size[0] * 4,
                ))
            }

            if let Some(tex) = &*last_texture {
                snapshot.append_scaled_texture(
                    tex,
                    self.filter.get(),
                    &Rect::new(0.0, 0.0, width as f32, height as f32),
                );
            } else {
                snapshot.append_color(
                    &gdk::RGBA::BLACK,
                    &graphene::Rect::new(0f32, 0f32, width as f32, height as f32),
                );
            }
        }
    }
}
