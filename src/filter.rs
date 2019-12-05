use arrayfire as AF;
use arrayfire::Array as AF_Array;
use arrayfire::Dim4 as AF_Dim4;

use std::sync::Mutex;
use std::collections::HashMap;

use crate::cli::DCNorm;
use crate::template::*;
use crate::filter_utils::*;

pub fn inner_product(
    templates: &[TemplateGroup],
    signals: &[Vec<f32>],
    signal_names: &[String],
    current_time: usize,
    // [ ] TODO include in calculations
    //  - ie work on estitmation, etc.
    _snf: f32,
    // [ ] TODO assume always on after template read
    //  - refactor out
    _pre_fft: bool,
    dc_norm: DCNorm,
    // [ ] TODO refactor into template instant.
    _template_group_len: usize,
    signal_group_len: usize,
) -> Vec<f32> {
    let mut res: Vec<f32> = Vec::new();
    for signals in signals.chunks(signal_group_len) {
        let signals = signals.to_vec();

        let signals = match dc_norm {
            DCNorm::MeanRemoveStar |
            DCNorm::MeanRemoveTemplateAndStar |
            DCNorm::NormAtZeroTemplateAndMeanRemoveStar => {
                stars_dc_removal(signals)
            }
            DCNorm::NormAtZeroStar |
            DCNorm::NormAtZeroTemplateAndStar |
            DCNorm::NormAtZeroStarAndMeanRemoveTemplate => {
                stars_norm_at_zero(signals)
            }
            DCNorm::HistMeanRemoveStar |
            DCNorm::HistMeanRemoveStarAndTemplate |
            DCNorm::HistMeanRemoveStarAndNormAtZeroTemplate => {
                let min_time = 30;
                let max_duration = 1200;
                stars_historical_mean_removal(signals, signal_names,
                                              min_time, max_duration, current_time)
            }
            _ => signals
        };

        let signals = outlier_removal_stars(signals);
        let signals = window_signals(signals, WindowFunc::Rectangle);
        let (stars, num_stars, signal_max_len) = stars_to_af(signals);

        let stars = stars_fft(&stars, templates[0].fft_len, templates[0].max_len);

        // [ ] TODO work on making right grouping
        //     of templates output and max of them
        //     -- for now only works bc large template groups (only one group)
        for template_group in templates {
            // [ ] TODO add in Delta x scale
            let res_af = AF::matmul(
                &stars,
                &template_group.templates,
                AF::MatProp::CTRANS,
                AF::MatProp::NONE,
            );

            // as in SO questions try using abs to get pos. vals.
            // https://{{so}}.com/questions/6740545/understanding-fft-output
            // https://dsp.{{se}}.com/questions/20500/negative-values-of-the-fft
            // --- can be fixed will describe in other doc
            //let res_af = AF::real(&res_af);
            //let res_af = AF::ifft(&res_af, 1.0, signal_max_len as i64);
            let res_af = AF::abs(&res_af);

            let res_af = AF::max(&res_af, 1);
            res_af.eval();

            let mut temp = af_to_vec1d(&res_af);
            res.append(&mut temp);
        }
    }

    res
}
