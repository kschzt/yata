#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::core::{IndicatorConfig, IndicatorInitializer, IndicatorInstance, IndicatorResult};
use crate::core::{Method, PeriodType, Source, ValueType, Window, OHLC};
use crate::methods::{Change, CrossAbove, CrossUnder};

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ChandeMomentumOscillator {
	pub period: PeriodType,
	pub zone: ValueType,
	pub source: Source,
}

impl IndicatorConfig for ChandeMomentumOscillator {
	fn validate(&self) -> bool {
		self.zone >= 0. && self.zone <= 1.0
	}

	fn set(&mut self, name: &str, value: String) {
		match name {
			"period" => self.period = value.parse().unwrap(),
			"zone" => self.zone = value.parse().unwrap(),
			"source" => self.source = value.parse().unwrap(),

			_ => {
				dbg!(format!(
					"Unknown attribute `{:}` with value `{:}` for `{:}`",
					name,
					value,
					std::any::type_name::<Self>(),
				));
			}
		};
	}

	fn size(&self) -> (u8, u8) {
		(1, 1)
	}
}

impl<T: OHLC> IndicatorInitializer<T> for ChandeMomentumOscillator {
	type Instance = ChandeMomentumOscillatorInstance;

	fn init(self, candle: T) -> Self::Instance
	where
		Self: Sized,
	{
		let cfg = self;

		Self::Instance {
			pos_sum: 0.,
			neg_sum: 0.,
			change: Change::new(1, candle.source(cfg.source)),
			window: Window::new(cfg.period, 0.),
			cross_under: CrossUnder::default(),
			cross_above: CrossAbove::default(),
			cfg: cfg,
		}
	}
}

impl Default for ChandeMomentumOscillator {
	fn default() -> Self {
		Self {
			period: 9,
			zone: 0.5,
			source: Source::Close,
		}
	}
}

#[derive(Debug)]
pub struct ChandeMomentumOscillatorInstance {
	cfg: ChandeMomentumOscillator,

	pos_sum: ValueType,
	neg_sum: ValueType,
	change: Change,
	window: Window<ValueType>,
	cross_under: CrossUnder,
	cross_above: CrossAbove,
}

#[inline]
fn change(change: ValueType) -> (ValueType, ValueType) {
	// let pos = if change > 0. { change } else { 0. };
	// let neg = if change < 0. { change * -1. } else { 0. };
	let pos = (change > 0.) as i8 as f64 * change;
	let neg = (change < 0.) as i8 as f64 * -change;

	(pos, neg)
}

impl<T: OHLC> IndicatorInstance<T> for ChandeMomentumOscillatorInstance {
	type Config = ChandeMomentumOscillator;

	#[inline]
	fn config(&self) -> &Self::Config {
		&self.cfg
	}

	fn next(&mut self, candle: T) -> IndicatorResult {
		let ch = self.change.next(candle.source(self.cfg.source));

		let left_value = self.window.push(ch);

		let (left_pos, left_neg) = change(left_value);
		let (right_pos, right_neg) = change(ch);

		self.pos_sum += right_pos - left_pos;
		self.neg_sum += right_neg - left_neg;

		let value = if self.pos_sum != 0. || self.neg_sum != 0. {
			(self.pos_sum - self.neg_sum) / (self.pos_sum + self.neg_sum)
		} else {
			0.
		};
		let signal = self.cross_under.next((value, -self.cfg.zone))
			- self.cross_above.next((value, self.cfg.zone));

		IndicatorResult::new(&[value], &[signal])
	}

	fn name(&self) -> &str {
		"ChandeMomentumOscillator"
	}
}