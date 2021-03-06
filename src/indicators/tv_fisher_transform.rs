// TradingView's Fisher Transform is slightly different to yata's.
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::core::{Action, Error, Method, PeriodType, Source, ValueType, OHLC};
use crate::core::{IndicatorConfig, IndicatorInitializer, IndicatorInstance, IndicatorResult};
use crate::helpers::{method, RegularMethod, RegularMethods};
use crate::methods::{Cross, Highest, Lowest};

// https://www.investopedia.com/terms/f/fisher-transform.asp
// FT = 1/2 * ln((1+x)/(1-x)) = arctanh(x)
// x - transformation of price to a level between -1 and 1 for N periods

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TVFisherTransform {
	pub period1: PeriodType,
	pub period2: PeriodType,
	pub zone: ValueType,
	pub delta: PeriodType,
	pub method: RegularMethods,
	pub source: Source,
}

impl IndicatorConfig for TVFisherTransform {
	const NAME: &'static str = "TVFisherTransform";

	fn validate(&self) -> bool {
		self.period1 >= 3 && self.delta >= 1 && self.period2 >= 1
	}

	fn set(&mut self, name: &str, value: String) -> Option<Error> {
		match name {
			"period1" => match value.parse() {
				Err(_) => return Some(Error::ParameterParse(name.to_string(), value.to_string())),
				Ok(value) => self.period1 = value,
			},
			"period2" => match value.parse() {
				Err(_) => return Some(Error::ParameterParse(name.to_string(), value.to_string())),
				Ok(value) => self.period2 = value,
			},
			"zone" => match value.parse() {
				Err(_) => return Some(Error::ParameterParse(name.to_string(), value.to_string())),
				Ok(value) => self.zone = value,
			},
			"delta" => match value.parse() {
				Err(_) => return Some(Error::ParameterParse(name.to_string(), value.to_string())),
				Ok(value) => self.delta = value,
			},
			"method" => match value.parse() {
				Err(_) => return Some(Error::ParameterParse(name.to_string(), value.to_string())),
				Ok(value) => self.method = value,
			},
			"source" => match value.parse() {
				Err(_) => return Some(Error::ParameterParse(name.to_string(), value.to_string())),
				Ok(value) => self.source = value,
			},

			_ => {
				return Some(Error::ParameterParse(name.to_string(), value));
			}
		};

		None
	}

	fn size(&self) -> (u8, u8) {
		(2, 2)
	}
}

impl<T: OHLC> IndicatorInitializer<T> for TVFisherTransform {
	type Instance = TVFisherTransformInstance;

	fn init(self, candle: T) -> Result<Self::Instance, Error>
	where
		Self: Sized,
	{
		if !self.validate() {
			return Err(Error::WrongConfig);
		}

		let cfg = self;
		let src = candle.source(cfg.source);

		Ok(Self::Instance {
			ma1: method(cfg.method, cfg.period2, 0.)?,
			highest: Highest::new(cfg.period1, src)?,
			lowest: Lowest::new(cfg.period1, src)?,
			cross_over: Cross::default(),
			extreme: 0,
			prev_value: 0.,
			prev_fish: 0.,
			prev_state: false,
			cfg,
		})
	}
}

impl Default for TVFisherTransform {
	fn default() -> Self {
		Self {
			period1: 9,
			period2: 1,
			zone: 1.5,
			delta: 1,
			method: RegularMethods::SMA,
			source: Source::TP,
		}
	}
}

#[derive(Debug)]
pub struct TVFisherTransformInstance {
	cfg: TVFisherTransform,

	ma1: RegularMethod,
	highest: Highest,
	lowest: Lowest,
	cross_over: Cross,
	extreme: i8,
	prev_value: ValueType,
	prev_fish: ValueType,
	prev_state: bool,
}

const BOUND: ValueType = 0.999;

#[inline]
fn bound_value(value: ValueType) -> ValueType {
	value.min(BOUND).max(-BOUND)
}

impl<T: OHLC> IndicatorInstance<T> for TVFisherTransformInstance {
	type Config = TVFisherTransform;

	fn config(&self) -> &Self::Config {
		&self.cfg
	}

	fn next(&mut self, candle: T) -> IndicatorResult {
		let src = candle.source(self.cfg.source);

		// converting original value to between -1.0 and 1.0 over period1
		let h = self.highest.next(src);
		let l = self.lowest.next(src);
		// we need to check division by zero, so we can really just check if `h` is equal to `l` without using any kind of round error checks
		#[allow(clippy::float_cmp)]
		let is_different = (h != l) as i8 as ValueType;
		let v1 = 0.66 * ((src - l) / (h - l + 1. - is_different) - 0.5) + 0.67 * self.prev_value;

		let bound_val = bound_value(v1);
		self.prev_value = v1;

		// calculating fisher transform value
		let fisher_transform: ValueType = 0.5 * ((1.0 + bound_val)/(1.0 - bound_val)).ln() + 0.5 * self.prev_fish;

		self.extreme =
			(fisher_transform < self.cfg.zone) as i8 - (fisher_transform > self.cfg.zone) as i8;

		let s1;
		{
			// We’ll take trade signals based on the following rules:
			// Long trades

			// 	Fisher Transform must be negative (i.e., the more negative the indicator is, the more “stretched” or excessively bearish price is)
			// 	Taken after a reversal of the Fisher Transform from negatively sloped to positively sloped (i.e., rate of change from negative to positive)

			// Short trades

			// 	Fisher Transform must be positive (i.e., price perceived to be excessively bullish)
			// 	Taken after a reversal in the direction of the Fisher Transform
			// s1 = if self.extreme == 1 && fisher_transform - self.prev_value < 0. {
			// 	-1
			// } else if self.extreme == -1 && fisher_transform - self.prev_value > 0. {
			// 	1
			// } else {
			// 	0
			// };
			s1 = (self.extreme == -1 && fisher_transform - self.prev_fish > 0.) as i8
				- (self.extreme == 1 && fisher_transform - self.prev_fish < 0.) as i8;
		}

		let s2;
		{
			// The Fisher Transform frequently has a signal line attached to it. This is a moving average of the Fisher Transform value,
			// so it moves slightly slower than the Fisher Transform line. When the Fisher Transform crosses the trigger line it is used
			// by some traders as a trade signal. For example, when the Fisher Transform drops below the signal line after hitting an
			// extreme high, that could be used as a signal to sell a current long position.
			let new_state = fisher_transform > self.prev_fish;
			let si = new_state as i8 * 2 - 1;
			s2 = if new_state != self.prev_state { si } else { 0 };
			self.prev_state = new_state;
		}

		let pf = self.prev_fish;
		self.prev_fish = fisher_transform;

		IndicatorResult::new(
			&[fisher_transform, pf],
			&[Action::from(s1), Action::from(s2)],
		)
	}
}
