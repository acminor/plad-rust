use arrayfire as AF;

use crate::cli::DCNorm;
use crate::filter_utils::*;
use crate::template::*;
use crate::utils;

enum DetectorType {
    Normal,
    DoubleSided,
    DoubleSidedWithMismatchNormalization,
    IFFT,
}

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
    window_func: WindowFunc,
    signal_group_len: usize,
) -> Vec<f32> {
    let mut res: Vec<f32> = Vec::new();
    for signals in signals.chunks(signal_group_len) {
        let signals = signals.to_vec();

        let signals = match dc_norm {
            DCNorm::MeanRemoveStar
            | DCNorm::MeanRemoveTemplateAndStar
            | DCNorm::NormAtZeroTemplateAndMeanRemoveStar => {
                stars_dc_removal(signals)
            }
            DCNorm::NormAtZeroStar
            | DCNorm::NormAtZeroTemplateAndStar
            | DCNorm::NormAtZeroStarAndMeanRemoveTemplate => {
                stars_norm_at_zero(signals)
            }
            DCNorm::HistMeanRemoveStar
            | DCNorm::HistMeanRemoveStarAndTemplate
            | DCNorm::HistMeanRemoveStarAndNormAtZeroTemplate => {
                let min_time = 30;
                let max_duration = 1200;
                //let signals = outlier_removal_stars(signals);
                stars_historical_mean_removal(
                    signals,
                    signal_names,
                    min_time,
                    max_duration,
                    current_time,
                    HistoricalMeanRunType::Fast,
                )
                //stars_min_max_historical_mean_removal(signals, signal_names,
                //                                      min_time, max_duration,
                //                                      current_time)
            }
            DCNorm::MeanRemoveConstBumpStarAndNormAtZeroTemplate => {
                // NOTE the bump is set to a value that means
                //      the final result should never be zero
                //      - thus the output should always be a positive signal
                stars_dc_removal_with_const(signals, 100.0)
            }
            _ => signals,
        };

        let signals = outlier_removal_stars(signals);
        let signals = window_signals(signals, window_func);
        let (stars, _num_stars, _signal_max_len) = stars_to_af(signals);

        let stars =
            stars_fft(&stars, templates[0].fft_len, templates[0].max_len);

        let detector_type = DetectorType::DoubleSided;

        let mut template_res = Vec::new();
        for template_group in templates {
            match detector_type {
                DetectorType::Normal => {
                    // [ ] TODO add in Delta x scale
                    // [ ] TODO make selection, but it does matter if templates
                    //     or stars gets conjugated verses the other (from observation)
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
                    //let res_af = AF::imag(&res_af);
                    //let res_af = AF::ifft(&res_af, 1.0, signal_max_len as i64);
                    let res_af = AF::abs(&res_af);

                    let res_af = AF::max(&res_af, 1);
                    template_res.push(res_af);
                    /*
                    res_af.eval();

                    let mut temp = af_to_vec1d(&res_af);
                    res.append(&mut temp);
                    */
                }
                /*
                 * This type of detector seems to eliminate all imaginary values
                 * forces the assumption that the functions are even ???
                 * [ ] TODO verify that the Single Sided Detector
                 *          has real and imaginary values
                 */
                DetectorType::DoubleSided => {
                    // [ ] TODO add in Delta x scale
                    // [ ] TODO make selection, but it does matter if templates
                    //     or stars gets conjugated verses the other (from observation)
                    let res_af_left = AF::matmul(
                        &stars,
                        &template_group.templates,
                        AF::MatProp::CTRANS,
                        AF::MatProp::NONE,
                    );

                    let res_af_right = AF::matmul(
                        &stars,
                        &AF::conjg(&template_group.templates),
                        AF::MatProp::TRANS,
                        AF::MatProp::NONE,
                    );

                    let res_af = AF::add(&res_af_left, &res_af_right, false);

                    // as in SO questions try using abs to get pos. vals.
                    // https://{{so}}.com/questions/6740545/understanding-fft-output
                    // https://dsp.{{se}}.com/questions/20500/negative-values-of-the-fft
                    // --- can be fixed will describe in other doc
                    let res_af = AF::real(&res_af);
                    //let res_af = AF::imag(&res_af);
                    //let res_af = AF::ifft(&res_af, 1.0, signal_max_len as i64);

                    /*
                     * NOTE: as a consequence of using the absolute value
                     *       certain values will be taken up that would not
                     *       be normally (for example any real value < 0.0).
                     *
                     *       This happened when the detector was choosing the max
                     *       value which was a close to 0 negative value but on switching
                     *       started to select a high-magnitude negative value since
                     *       under abs it would be positive.
                     */
                    //let res_af = AF::abs(&res_af);

                    let res_af = AF::max(&res_af, 1);
                    template_res.push(res_af);
                    /*
                    res_af.eval();

                    let mut temp = af_to_vec1d(&res_af);
                    res.append(&mut temp);
                    */
                }
                DetectorType::DoubleSidedWithMismatchNormalization => {
                    // [ ] TODO add in Delta x scale
                    // [ ] TODO make selection, but it does matter if templates
                    //     or stars gets conjugated verses the other (from observation)

                    let diff_norm_factor = AF::matmul(
                        &stars,
                        &AF::mul(&template_group.templates, &(-1.0 as f32), false),
                        AF::MatProp::TRANS,
                        AF::MatProp::NONE,
                    );
                    let diff_norm_factor = AF::add(&diff_norm_factor, &(1.0 as f32), false);

                    let res_af_left = AF::matmul(
                        &stars,
                        &template_group.templates,
                        AF::MatProp::CTRANS,
                        AF::MatProp::NONE,
                    );

                    let res_af_right = AF::matmul(
                        &stars,
                        &AF::conjg(&template_group.templates),
                        AF::MatProp::TRANS,
                        AF::MatProp::NONE,
                    );

                    let res_af = AF::add(&res_af_left, &res_af_right, false);

                    let res_af = AF::div(&res_af, &diff_norm_factor, false);

                    // as in SO questions try using abs to get pos. vals.
                    // https://{{so}}.com/questions/6740545/understanding-fft-output
                    // https://dsp.{{se}}.com/questions/20500/negative-values-of-the-fft
                    // --- can be fixed will describe in other doc
                    let res_af = AF::real(&res_af);
                    //let res_af = AF::imag(&res_af);
                    //let res_af = AF::ifft(&res_af, 1.0, signal_max_len as i64);

                    /*
                     * NOTE: as a consequence of using the absolute value
                     *       certain values will be taken up that would not
                     *       be normally (for example any real value < 0.0).
                     *
                     *       This happened when the detector was choosing the max
                     *       value which was a close to 0 negative value but on switching
                     *       started to select a high-magnitude negative value since
                     *       under abs it would be positive.
                     */
                    //let res_af = AF::abs(&res_af);

                    let res_af = AF::max(&res_af, 1);
                    template_res.push(res_af);
                    /*
                    res_af.eval();

                    let mut temp = af_to_vec1d(&res_af);
                    res.append(&mut temp);
                    */
                }
                // NOTE not actually IFFT
                DetectorType::IFFT => {
                    let mut star_temp_ress = Vec::new();
                    //println!("SD: {}", stars.dims());
                    let star_num_rows = stars.dims()[0];
                    let star_num_cols = stars.dims()[1];
                    let temp_num_cols = template_group.templates.dims()[1];
                    for i in 0..star_num_rows {
                        let cur_star = AF::row(&stars, i);
                        //AF::print(&cur_star);
                        let cur_star = AF::tile(&cur_star,
                                                AF::Dim4::new(
                                                    &[star_num_rows, temp_num_cols, 1, 1]));
                        //println!("star_dims {}", cur_star.dims());
                        //println!("temp_dims {}", template_group.templates.dims());
                        let temp_res = AF::sub(&cur_star, &template_group.templates, false);
                        //AF::print(&temp_res);
                        let temp_res = AF::sum(&temp_res, 1);

                        //println!("Here");
                        let temp_res = AF::min(&temp_res, 0);
                        //AF::print(&temp_res);
                        //println!("Here");
                        star_temp_ress.push(temp_res);
                    }

                    let mut iter = star_temp_ress.into_iter();
                    let mut star_final_res = iter.next()
                        .expect("Should have at least one set of results.");
                    for val in iter {
                        star_final_res = AF::join(0, &star_final_res, &val);
                    }

                    // NOTE Convert to a maximum problem
                    let star_final_res = AF::real(&star_final_res);
                    let star_final_res = AF::sub(&(100.0 as f32), &star_final_res, false);
                    println!("Here");
                    template_res.push(star_final_res);
                }
            }
        }

        // Joins temporary star template matchings together
        // into one master result for export (global template maximum)
        let mut iter = template_res.into_iter();
        let mut final_res = iter.next()
            .expect("Should have at least one set of results.");
        for group in iter {
            final_res = AF::join(1, &final_res, &group);
        }

        let final_res = AF::max(&final_res, 1);
        final_res.eval();

        let mut temp = af_to_vec1d(&final_res);
        res.append(&mut temp);
    }

    res
}
