use std::fmt;

use crate::ParseError;

/// Defines a header enum with a strict `TryFrom<i32>`: values that are not listed are an error.
macro_rules! header_enum {
    (
        $(#[$meta:meta])*
        $name:ident, $key:literal {
            $($variant:ident = $value:literal => $label:literal),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[repr(i32)]
        pub enum $name {
            $($variant = $value),*
        }

        impl $name {
            /// The raw value as stored in the file header.
            pub fn as_i32(self) -> i32 {
                self as i32
            }
        }

        impl TryFrom<i32> for $name {
            type Error = ParseError;

            fn try_from(value: i32) -> Result<Self, ParseError> {
                match value {
                    $($value => Ok(Self::$variant),)*
                    _ => Err(ParseError::invalid($key, format!("unknown value {value}"))),
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(match self {
                    $(Self::$variant => $label,)*
                })
            }
        }
    };
}

header_enum! {
    /// What kind of data the file contains (`DATAMODE_*`).
    Datamode, "datamode" {
        Undef = -1 => "UNDEF",
        CounterUnused = 0 => "COUNTER_UNUSED",
        Signal = 1 => "SIGNAL",
        Fft = 2 => "FFT",
        Plot = 3 => "PLOT",
        Other = 4 => "OTHER",
        Video = 5 => "VIDEO",
        Count = 6 => "COUNT",
    }
}

header_enum! {
    /// How the data was compressed / aggregated (`COMP_*`). `Raw` is uncompressed hardware data.
    Datatype, "datatype" {
        Invalid = -2 => "INVALID",
        Undef = -1 => "UNDEF",
        Raw = 0 => "RAW",
        Downsample = 1 => "DOWNSAMPLE",
        Maximum = 2 => "MAXIMUM",
        Average = 3 => "AVERAGE",
        StdDeviation = 4 => "STD_DEVIATION",
        Envelope = 5 => "ENVELOPE",
        MovingAverage = 6 => "MOV_AVERAGE",
        ExternalData = 7 => "EXTERN_DATA",
        AnalyzeOverview = 8 => "ANALYZE_OVERVIEW",
        MovingAverageOpt = 9 => "MOV_AVERAGE_OPT",
        Collection = 10 => "COLLECTION",
        ImportSig = 11 => "IMPORT_SIG",
        ScopeRaw = 12 => "SCOPE_RAW",
        MovingAverageFrq = 13 => "MOV_AVERAGE_FRQ",
        SlopeChange = 14 => "SLOPECHANGE",
        Other = 15 => "OTHER",
        IoSignal = 16 => "IO_SIGNAL",
        Energy = 17 => "ENERGY",
        AudioRaw = 18 => "AUDIO_RAW",
        XyPlot = 19 => "XY_PLOT",
        SecondFft = 20 => "SECOND_FFT",
        AudioComment = 21 => "AUDIO_COMMENT",
        CpEnergySig = 22 => "CP_ENERGY_SIG",
        VideoMeasure = 23 => "VID_MEASURE",
        VideoScreen = 24 => "VID_SCREEN",
        VideoExtLink = 25 => "VID_EXT_LINK",
        EnvelopeUpper = 26 => "ENVELOPE_UPPER",
        EnvelopeLower = 27 => "ENVELOPE_LOWER",
        PatternRefObj = 28 => "PATTERN_REFOBJ",
        Significance = 29 => "SIGNIFICANCE",
        Significance32 = 30 => "SIGNIFICANCE_32",
        PatternMask = 31 => "PATTERN_MASK",
        GradientFrq = 32 => "GRADIENT_FRQ",
        CsvExport = 33 => "CSV_EXPORT",
        Count = 34 => "COUNT",
    }
}

header_enum! {
    /// Additional specification of the file (`KIND_*`).
    Datakind, "datakind" {
        Undef = -1 => "UNDEF",
        None = 0 => "NONE",
        SensorTest = 1 => "SENSOR_TEST",
        FreeDatablockTiming = 2 => "PLOT_FREE_DATABLOCK_TIMING",
        PatternRefObjCompr = 3 => "PATTERN_REF_OBJ_COMPR",
        PatternRefObjMask = 4 => "PATTERN_REF_OBJ_MASK",
        PatternRefObjExtra = 5 => "PATTERN_REF_OBJ_EXTRA",
        Free6 = 6 => "FREE_6",
        Free7 = 7 => "FREE_7",
        Count = 8 => "COUNT",
        User = 100 => "USER",
    }
}

header_enum! {
    /// The analog digital converter used for the measurement. `NotUsed` is also the legacy 14 bit ADC.
    AdcType, "adc_type" {
        NotUsed = 0 => "NOT_USED",
        Adc16Bit = 1 => "16BIT",
        Adc24Bit = 2 => "24BIT",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_values_convert() {
        assert_eq!(Datamode::try_from(2).unwrap(), Datamode::Fft);
        assert_eq!(Datatype::try_from(-2).unwrap(), Datatype::Invalid);
        assert_eq!(Datakind::try_from(100).unwrap(), Datakind::User);
        assert_eq!(AdcType::try_from(1).unwrap(), AdcType::Adc16Bit);
        assert_eq!(Datatype::Raw.as_i32(), 0);
        assert_eq!(Datamode::Fft.to_string(), "FFT");
    }

    #[test]
    fn unknown_values_are_errors() {
        assert!(Datamode::try_from(7).is_err());
        assert!(Datatype::try_from(35).is_err());
        assert!(Datakind::try_from(9).is_err());
        assert!(AdcType::try_from(3).is_err());
    }
}
