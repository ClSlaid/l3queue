use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use charts::{AxisPosition, Chart, Color, LineSeriesView, MarkerType, ScaleLinear};
use l3queue::{crs_queue::CrsQueue, lq::LinkedQueue, mutex_queue::MutexQueue};

fn main() {
    let _du = 30;
    let begin = Instant::now();
    let du = Duration::from_secs(_du);
    let epoch = Duration::from_secs(1);
    let ddl = begin + du;

    let p_lq_cnt = Arc::new(AtomicUsize::new(0));
    let p_lq_cnt1 = p_lq_cnt.clone();

    let p_mq_cnt = Arc::new(AtomicUsize::new(0));
    let p_mq_cnt1 = p_mq_cnt.clone();

    let p_cq_cnt = Arc::new(AtomicUsize::new(0));
    let p_cq_cnt1 = p_cq_cnt.clone();

    let p_lq = Arc::new(LinkedQueue::new());
    let c_lq = p_lq.clone();
    let p_mq = Arc::new(MutexQueue::new());
    let c_mq = p_mq.clone();
    let p_cq = Arc::new(CrsQueue::new());
    let c_cq = p_cq.clone();

    let _t1 = thread::spawn(move || {
        for i in 0u128.. {
            p_lq.push(i);
            p_lq_cnt1.fetch_add(1, Ordering::Release);
        }
    });
    let _t2 = thread::spawn(move || loop {
        loop {
            c_lq.pop();
        }
    });
    let _t3 = thread::spawn(move || {
        for i in 0u128.. {
            p_mq.push(i);
            p_mq_cnt1.fetch_add(1, Ordering::Release);
        }
    });
    let _t4 = thread::spawn(move || loop {
        loop {
            c_mq.pop();
        }
    });

    let _t5 = thread::spawn(move || {
        for i in 0u128.. {
            p_cq.push(i);
            p_cq_cnt1.fetch_add(1, Ordering::Release);
        }
    });
    let _t6 = thread::spawn(move || loop {
        loop {
            c_cq.pop();
        }
    });
    let lq_p = p_lq_cnt.load(Ordering::Acquire);
    let cq_p = p_cq_cnt.load(Ordering::Acquire);
    let mq_p = p_mq_cnt.load(Ordering::Acquire);

    let mut lq_full = vec![lq_p];
    let mut cq_full = vec![cq_p];
    let mut mq_full = vec![mq_p];
    let mut lq = vec![];
    let mut cq = vec![];
    let mut mq = vec![];

    println!("start recording...");
    println!("time,bw_lq,bw_cq,bw_mq");
    let mut now = Instant::now();
    while now <= ddl {
        thread::sleep(epoch);
        now = Instant::now();

        let uptime = now.duration_since(begin).as_secs();

        let lq_p = p_lq_cnt.load(Ordering::Acquire);
        let cq_p = p_cq_cnt.load(Ordering::Acquire);
        let mq_p = p_mq_cnt.load(Ordering::Acquire);

        let bw_lq = lq_p - lq_full[lq_full.len() - 1];
        let bw_cq = cq_p - cq_full[cq_full.len() - 1];
        let bw_mq = mq_p - mq_full[mq_full.len() - 1];
        println!("{},{},{},{}", uptime, bw_lq, bw_cq, bw_mq);
        lq_full.push(lq_p);
        cq_full.push(cq_p);
        mq_full.push(mq_p);
        lq.push(bw_lq);
        cq.push(bw_cq);
        mq.push(bw_mq);
    }

    let max = lq
        .iter()
        .max()
        .unwrap()
        .max(cq.iter().max().unwrap())
        .max(mq.iter().max().unwrap());
    let range = max / 5 * 6; // 120%

    let width = 800;
    let height = 600;
    let (top, right, bottom, left) = (90, 40, 50, 110);
    let x = ScaleLinear::new()
        .set_domain(vec![0f32, _du as f32])
        .set_range(vec![0, width - left - right]);
    let y = ScaleLinear::new()
        .set_domain(vec![0f32, range as f32])
        .set_range(vec![height - top - bottom, 0]);

    let lq_data = (1..=_du)
        .map(|x| x as f32)
        .zip(lq.iter().map(|&x| x as f32))
        .collect();
    let lq_view = LineSeriesView::new()
        .set_x_scale(&x)
        .set_y_scale(&y)
        .set_marker_type(MarkerType::Circle)
        .set_label_visibility(false)
        .set_colors(Color::from_vec_of_hex_strings(vec!["#FF4700"]))
        .set_custom_data_label(String::from("手写链表实现"))
        .load_data(&lq_data)
        .unwrap();
    let cq_data = (1..=_du)
        .map(|x| x as f32)
        .zip(cq.iter().map(|&x| x as f32))
        .collect();
    let cq_view = LineSeriesView::new()
        .set_x_scale(&x)
        .set_y_scale(&y)
        .set_marker_type(MarkerType::Square)
        .set_label_visibility(false)
        .set_colors(Color::from_vec_of_hex_strings(vec!["#47FF00"]))
        .set_custom_data_label(String::from("Crossbeam GC 链表实现"))
        .load_data(&cq_data)
        .unwrap();
    let mq_data = (1..=_du)
        .map(|x| x as f32)
        .zip(mq.iter().map(|&x| x as f32))
        .collect();
    let mq_view = LineSeriesView::new()
        .set_x_scale(&x)
        .set_y_scale(&y)
        .set_marker_type(MarkerType::X)
        .set_label_visibility(false)
        .set_colors(Color::from_vec_of_hex_strings(vec!["#0047FF"]))
        .set_custom_data_label(String::from("链表加大锁实现"))
        .load_data(&mq_data)
        .unwrap();

    Chart::new()
        .set_width(width)
        .set_height(height)
        .set_margins(top, right, bottom, left)
        .add_title(String::from("带宽测试"))
        .add_view(&lq_view)
        .add_view(&cq_view)
        .add_view(&mq_view)
        .add_axis_bottom(&x)
        .add_axis_left(&y)
        .add_left_axis_label("带宽（个）")
        .add_bottom_axis_label("时间（秒）")
        .add_legend_at(AxisPosition::Bottom)
        .save("line-chart.svg")
        .unwrap();
}
