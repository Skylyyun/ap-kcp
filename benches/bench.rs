use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use rand::prelude::*;
use smol::{net::UdpSocket, prelude::*};
use std::{fs::File, sync::Arc};

pub const DATA_SIZE: usize = 0x1000000 * 4; // 64 MB

pub async fn get_udp_pair() -> (UdpSocket, UdpSocket) {
    let io1 = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let io2 = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    io1.connect(io2.local_addr().unwrap()).await.unwrap();
    io2.connect(io1.local_addr().unwrap()).await.unwrap();
    (io1, io2)
}

fn random_data() -> Arc<Vec<u8>> {
    let mut buf = Vec::new();
    buf.resize(DATA_SIZE, 0);
    rand::thread_rng().fill_bytes(&mut buf);
    Arc::new(buf)
}

fn init() {
    std::env::set_var("SMOL_THREADS", "8");
}

fn udp(data: Arc<Vec<u8>>) {
    smol::block_on(async move {
        let (io1, io2) = get_udp_pair().await;
        let handle1 = ap_kcp::KcpHandle::new(io1, ap_kcp::KcpConfig::default());
        let data1 = data.clone();
        let t = smol::spawn(async move {
            let handle2 = ap_kcp::KcpHandle::new(io2, ap_kcp::KcpConfig::default());
            let mut stream2 = handle2.accept().await.unwrap();
            let mut buf = Vec::new();
            buf.resize(data1.len(), 0);
            stream2.read_exact(&mut buf).await.unwrap();
        });
        let mut stream1 = handle1.connect().await.unwrap();
        stream1.write_all(&data).await.unwrap();
        t.await;
    });
}

pub fn xmit_benchmark(c: &mut Criterion) {
    init();
    let data = random_data();
    let mut group = c.benchmark_group("xmit");
    group.throughput(Throughput::Bytes(DATA_SIZE as u64));
    group.bench_function("udp", |b| b.iter(|| udp(data.clone())));

    {
        let guard = pprof::ProfilerGuard::new(1000).unwrap();
        if let Ok(report) = guard.report().build() {
            println!("report: {}", &report);
        };
        group.bench_function("udp-flamegraph", |b| b.iter(|| udp(data.clone())));
        if let Ok(report) = guard.report().build() {
            let file = File::create("flamegraph.svg").unwrap();
            report.flamegraph(file).unwrap();
        };
    }

    group.finish();
}

criterion_group! {
    name = handshake_benches;
    config = Criterion::default().sample_size(10);
    targets = xmit_benchmark
}

criterion_main!(handshake_benches);
