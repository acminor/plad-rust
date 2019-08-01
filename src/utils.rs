use arrayfire as AF;
use arrayfire::device_mem_info;
use arrayfire::mem_info;
use arrayfire::Array as AF_Array;
use arrayfire::Dim4 as AF_Dim4;
use gnuplot::{Color, Figure};
use num::Complex;
use rustfft::{
    num_complex::Complex as fft_complex, num_traits::Zero, FFTplanner,
};

pub fn inner_product(
    //templates: &Vec<Vec<Complex<f32>>>,
    templates: &AF_Array<Complex<f32>>,
    signals: &Vec<Vec<f32>>,
    snf: f32,
    pre_fft: bool,
    template_group_len: usize,
    signal_group_len: usize,
) -> Vec<f32> {
    //let width = templates[0].len();
    let mut res: Vec<f32> = Vec::new();
    for signals in signals.chunks(signal_group_len) {
        //for templates in templates.chunks(template_group_len) {
        let signals = &signals
            .iter()
            .flat_map(|signal| signal.into_iter())
            .cloned()
            .collect::<Vec<f32>>()[..];
        let stars = AF_Array::new(
            signals,
            AF_Dim4::new(&[30 as u64, (signals.len() / 30) as u64, 1, 1]),
        );

        let stars = AF::fft(&stars, 1.0, 42287 as i64);
        let stars = AF::transpose(&stars, false);
        let templates = AF::transpose(&templates, false);
        let res_af = AF::matmul(
            &stars,
            &templates,
            AF::MatProp::NONE,
            AF::MatProp::NONE,
        );

        let mut temp: Vec<f32> = Vec::new();
        temp.resize(res_af.elements(), 0.0);
        let res_af = AF::real(&res_af);
        res_af.lock();
        res_af.host(&mut temp);
        res_af.unlock();

        res.append(&mut temp);
    }

    /*
        if false {
            let dbg_data: Vec<f32> = template.iter().map(|&x| x.re).collect();
            let mut fg = Figure::new();
            fg.axes2d()
                .lines(0..template.len(), dbg_data, &[Color("black")]);
            fg.show();
        }
    k   */

    res
}
