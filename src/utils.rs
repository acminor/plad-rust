use gnuplot::{Color, Figure};
use rustfft::{num_complex::Complex, num_traits::Zero, FFTplanner};

pub fn inner_product(
    template: &Vec<Complex<f32>>,
    signal: &Vec<f32>,
    snf: f32,
    pre_fft: bool,
) -> Vec<Complex<f32>> {
    let size = match template.len() > signal.len() {
        true => template.len(),
        false => signal.len(),
    };

    // TODO pre fft

    let mut input: Vec<Complex<f32>> = signal
        .clone()
        .into_iter()
        .map(|x| Complex::new(x, 0.0))
        .collect();
    let mut output = vec![Complex::zero(); signal.len()];

    let mut planner = FFTplanner::new(false);
    let fft = planner.plan_fft(signal.len());
    fft.process(&mut input, &mut output);

    if false {
        let dbg_data: Vec<f32> = template.iter().map(|&x| x.re).collect();
        let mut fg = Figure::new();
        fg.axes2d()
            .lines(0..template.len(), dbg_data, &[Color("black")]);
        fg.show();
    }

    let num = output
        .into_iter()
        .zip(template.into_iter())
        .map(|(x, y)| x * y);
    let eq = num.map(|x| snf * x).collect();

    eq
}
