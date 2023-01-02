use crate::{
	enums::{Direction, Effects, Message},
	keyboard_utils,
	profile::Profile,
};
use crate::{
	error,
	keyboard_utils::{BaseEffects, Keyboard},
};

use crossbeam_channel::{Receiver, Sender};
use rand::thread_rng;
use std::{
	sync::atomic::{AtomicBool, Ordering},
	thread,
	time::Duration,
};
use std::{sync::Arc, thread::JoinHandle};

use self::{
	ambient::AmbientLight,
	christmas::Christmas,
	custom_effect::{CustomEffect, EffectType},
	disco::Disco,
	fade::Fade,
	lightning::Lightning,
	ripple::Ripple,
	swipe::Swipe,
	temperature::Temperature,
};

mod ambient;
mod christmas;
pub mod custom_effect;
mod disco;
mod fade;
mod lightning;
mod ripple;
mod swipe;
mod temperature;

/// Manager wrapper
pub struct EffectManager {
	pub tx: Sender<Message>,
	inner_handle: JoinHandle<()>,
}

/// Controls the keyboard lighting logic
struct Inner {
	keyboard: Keyboard,
	tx: Sender<Message>,
	rx: Receiver<Message>,
	stop_signals: StopSignals,
	last_profile: Profile,
}

impl EffectManager {
	pub fn new() -> Result<Self, error::Error> {
		let stop_signals = StopSignals {
			manager_stop_signal: Arc::new(AtomicBool::new(false)),
			keyboard_stop_signal: Arc::new(AtomicBool::new(false)),
		};

		let keyboard = keyboard_utils::get_keyboard(stop_signals.keyboard_stop_signal.clone())?;

		let (tx, rx) = crossbeam_channel::unbounded::<Message>();

		let mut inner = Inner {
			keyboard,
			rx,
			tx: tx.clone(),
			stop_signals,
			last_profile: Profile::default(),
		};

		let inner_handle = thread::spawn(move || loop {
			match inner.rx.try_recv().ok() {
				Some(message) => match message {
					Message::Refresh => {
						inner.refresh();
					}
					Message::Profile { profile } => {
						inner.last_profile = profile;
						inner.set_profile(profile);
					}
					Message::CustomEffect { effect } => {
						inner.custom_effect(effect);
					}
					Message::Exit => break,
				},
				None => {
					thread::sleep(Duration::from_millis(20));
				}
			}
		});

		let manager = Self { tx, inner_handle };

		Ok(manager)
	}

	pub fn set_profile(&self, profile: Profile) {
		self.tx.try_send(Message::Profile { profile }).unwrap();
	}

	pub fn custom_effect(&self, effect: CustomEffect) {
		self.tx.send(Message::CustomEffect { effect }).unwrap();
	}

	pub fn join_and_exit(self) {
		self.tx.send(Message::Exit).unwrap();
		self.inner_handle.join().unwrap();
	}
}

impl Inner {
	fn refresh(&mut self) {
		self.set_profile(self.last_profile);
	}

	fn set_profile(&mut self, mut profile: Profile) {
		self.stop_signals.store_false();
		let mut thread_rng = thread_rng();

		self.keyboard.set_effect(BaseEffects::Static);
		self.keyboard.set_speed(profile.speed);
		self.keyboard.set_brightness(profile.brightness);

		match profile.effect {
			Effects::Static => {
				self.keyboard.set_colors_to(&profile.rgb_array);
				self.keyboard.set_effect(BaseEffects::Static);
			}
			Effects::Breath => {
				self.keyboard.set_colors_to(&profile.rgb_array);
				self.keyboard.set_effect(BaseEffects::Breath);
			}
			Effects::Smooth => {
				self.keyboard.set_effect(BaseEffects::Smooth);
			}
			Effects::Wave => match profile.direction {
				Direction::Left => self.keyboard.set_effect(BaseEffects::LeftWave),
				Direction::Right => self.keyboard.set_effect(BaseEffects::RightWave),
			},

			Effects::Lightning => Lightning::play(self, profile, &mut thread_rng),
			Effects::AmbientLight { fps } => AmbientLight::play(self, fps),
			Effects::SmoothWave => {
				profile.rgb_array = [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 0, 255];
				Swipe::play(self, profile)
			}
			Effects::Swipe => Swipe::play(self, profile),
			Effects::Disco => Disco::play(self, profile, &mut thread_rng),
			Effects::Christmas => Christmas::play(self, &mut thread_rng),
			Effects::Fade => Fade::play(self, profile),
			Effects::Temperature => Temperature::play(self),
			Effects::Ripple => Ripple::play(self, profile),
		}
		self.stop_signals.store_false();
	}

	fn custom_effect(&mut self, custom_effect: CustomEffect) {
		self.stop_signals.store_false();

		'outer: loop {
			for step in custom_effect.effect_steps.clone() {
				self.keyboard.set_speed(step.speed);
				self.keyboard.set_brightness(step.brightness);
				if let EffectType::Set = step.step_type {
					self.keyboard.set_colors_to(&step.rgb_array);
				} else {
					self.keyboard.transition_colors_to(&step.rgb_array, step.steps, step.delay_between_steps);
				}
				if self.stop_signals.manager_stop_signal.load(Ordering::SeqCst) {
					break 'outer;
				}
				thread::sleep(Duration::from_millis(step.sleep));
			}
			if !custom_effect.should_loop {
				break;
			}
		}
	}
}
#[derive(Clone)]
pub struct StopSignals {
	pub manager_stop_signal: Arc<AtomicBool>,
	pub keyboard_stop_signal: Arc<AtomicBool>,
}

impl StopSignals {
	pub fn store_true(&self) {
		self.keyboard_stop_signal.store(true, Ordering::SeqCst);
		self.manager_stop_signal.store(true, Ordering::SeqCst);
	}
	pub fn store_false(&self) {
		self.keyboard_stop_signal.store(false, Ordering::SeqCst);
		self.manager_stop_signal.store(false, Ordering::SeqCst);
	}
}
