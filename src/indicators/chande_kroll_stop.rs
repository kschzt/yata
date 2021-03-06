#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
// use std::str::FromStr;

use crate::core::{Action, Error, Method, PeriodType, Source, ValueType, OHLC};
use crate::core::{IndicatorConfig, IndicatorInitializer, IndicatorInstance, IndicatorResult};
use crate::helpers::{method, signi, RegularMethod, RegularMethods};
use crate::methods::{CrossAbove, Highest, Lowest};

/// Chande Kroll Stop
///
/// ## Links
///
/// * <https://tradingview.com/support/solutions/43000589105-chande-kroll-stop/>
///
/// # 3 values
///
/// * `stop long` \(range of values is the same as range of the `source` values\)
/// * `source` value
/// * `stop short` \(range of values is the same as range of the `source` values\)
///
/// # 2 signals
///
/// * signal 1 is calculated according to relative position of the `source` value between `stop short` and `stop long` values.
///
/// When `source` value goes above `stop short`, then returns full buy signal.
///
/// When `source` value goes below `stop long`, then returns full sell signal.
/// * signal 2 appears only when `stop long` crosses `stop short` upwards.
///
/// When cumulative move of `stop short` and `stop long` is upwards, then returns full buy.
///
/// When cumulative move of `stop short` and `stop long` is downwards, then returns full sell.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ChandeKrollStop {
	/// ATR period length. Default is 10.
	///
	/// Range in \[1; [`PeriodType::MAX`](crate::core::PeriodType)\]
	pub p: PeriodType,
	/// ATR method. Default is [`SMA`](crate::methods::SMA).
	pub method: RegularMethods,
	/// ATR multiplier. Default is 1.0.
	///
	/// Range in \[0; +inf\)
	pub x: ValueType,
	/// multiplied highest/lowest period length. Default is 9.
	///
	/// Range in \[1; [`PeriodType::MAX`](crate::core::PeriodType)\]
	pub q: PeriodType,
	/// price source. Default is [`Close`](crate::core::Source#variant.Close)
	pub source: Source,
}

impl IndicatorConfig for ChandeKrollStop {
	const NAME: &'static str = "ChandeKrollStop";

	fn validate(&self) -> bool {
		self.x >= 0.0 && self.p > 0 && self.q > 0
	}

	fn set(&mut self, name: &str, value: String) -> Option<Error> {
		match name {
			"p" => match value.parse() {
				Err(_) => return Some(Error::ParameterParse(name.to_string(), value.to_string())),
				Ok(value) => self.p = value,
			},
			"x" => match value.parse() {
				Err(_) => return Some(Error::ParameterParse(name.to_string(), value.to_string())),
				Ok(value) => self.x = value,
			},
			"q" => match value.parse() {
				Err(_) => return Some(Error::ParameterParse(name.to_string(), value.to_string())),
				Ok(value) => self.q = value,
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
		(3, 2)
	}
}

impl<T: OHLC> IndicatorInitializer<T> for ChandeKrollStop {
	type Instance = ChandeKrollStopInstance<T>;

	fn init(self, candle: T) -> Result<Self::Instance, Error>
	where
		Self: Sized,
	{
		if !self.validate() {
			return Err(Error::WrongConfig);
		}

		let tr = candle.high() - candle.low();

		let cfg = self;
		Ok(Self::Instance {
			ma: method(cfg.method, cfg.p, candle.tr(&candle))?,

			highest1: Highest::new(cfg.p, candle.high())?,
			lowest1: Lowest::new(cfg.p, candle.low())?,

			highest2: Highest::new(cfg.q, candle.high() - cfg.x * tr)?,
			lowest2: Lowest::new(cfg.q, cfg.x.mul_add(tr, candle.low()))?,

			prev_candle: candle,
			prev_stop_short: candle.high() - cfg.x * tr,
			prev_stop_long: cfg.x.mul_add(tr, candle.low()),
			cross_above: CrossAbove::new(
				(),
				(cfg.x.mul_add(tr, candle.low()), candle.high() - cfg.x * tr),
			)?,
			cfg,
		})
	}
}

impl Default for ChandeKrollStop {
	fn default() -> Self {
		Self {
			p: 10,
			method: RegularMethods::SMA,
			x: 1.0,
			q: 9,
			source: Source::Close,
		}
	}
}

/// Chande Kroll Stop state structure
#[derive(Debug)]
pub struct ChandeKrollStopInstance<T: OHLC> {
	cfg: ChandeKrollStop,

	ma: RegularMethod,
	highest1: Highest,
	lowest1: Lowest,
	highest2: Highest,
	lowest2: Lowest,
	prev_candle: T,
	prev_stop_short: ValueType,
	prev_stop_long: ValueType,
	cross_above: CrossAbove,
}

impl<T: OHLC> IndicatorInstance<T> for ChandeKrollStopInstance<T> {
	type Config = ChandeKrollStop;

	fn config(&self) -> &Self::Config {
		&self.cfg
	}

	#[allow(clippy::similar_names)]
	fn next(&mut self, candle: T) -> IndicatorResult {
		let tr = candle.tr(&self.prev_candle);
		self.prev_candle = candle;

		let atr = self.ma.next(tr);

		let phs = self.highest1.next(candle.high()) - atr * self.cfg.x;
		let pls = atr.mul_add(self.cfg.x, self.lowest1.next(candle.low()));

		let stop_short = self.highest2.next(phs);
		let stop_long = self.lowest2.next(pls);

		let src = candle.source(self.cfg.source);

		let mid = (stop_short + stop_long) * 0.5;
		let size = mid - stop_long;

		let value = if size == 0.0 { 0.0 } else { (src - mid) / size };

		#[allow(unused_parens)]
		let s2_diff = (stop_short - self.prev_stop_short) + (stop_long - self.prev_stop_long);
		let is_s2 = (stop_short < stop_long) as i8; // s2 should appear only when `STOP LONG` is above `STOP SHORT`
		let cross: i8 = self.cross_above.next((stop_long, stop_short)).into(); // also s2 should appear only when `STOP LONG` actually crossing `STOP SHORT` upwards
		let s2 = cross * is_s2 * signi(s2_diff);

		IndicatorResult::new(
			&[stop_long, src, stop_short],
			&[Action::from(value), Action::from(s2)],
		)
	}
}
