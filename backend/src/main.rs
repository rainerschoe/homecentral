pub mod data_lake;
use data_lake::*;
//use tokio_stream::stream_ext::StreamExt;
//use bus::MessagePackBusAccess;
//use bus::ConnectionSetup;


pub mod bus {
    tonic::include_proto!("bus");
}
//use futures::stream::Stream;
use tokio_stream::StreamExt;

async fn receive_from_bus_and_publish(datalake: TDataLake, server_url: String, publish_base_path: String)
{
    let mut client = bus::message_pack_bus_access_client::MessagePackBusAccessClient::connect(server_url).await.unwrap();

    //let request = tonic::Request::new(bus::ConnectionSetup {
    //    rx_mask: "255:255".into(),
    //    remote_address: "255:255".into(),
    //});
    let req = bus::ConnectionSetup {
        rx_mask: "255:255".into(),
        remote_address: "200:1".into(),
    };
    let response = client.receive(req).await.unwrap();

    let mut resp_stream = response.into_inner();
    while let Some(received) = resp_stream.next().await {
        let received = received.unwrap();
        println!("\treceived message: `{:?}`", received);
    }


}

#[tokio::main]
async fn main() -> ()
{
    let mut datalake = TDataLake::new();
    receive_from_bus_and_publish(datalake, "ipv4://192.168.0.200:50051".into(), "bus/receive/ug".into()).await;
}

#[tokio::test]
async fn multi_task_publish_subscribe()
{
    let mut datalake = TDataLake::new();
    let datalake2 = datalake.clone();


    let join1 = tokio::task::spawn(async move {
        let mut sub = datalake.subscribe::<String>(&"/test".parse().unwrap()).await;
        for i in 1..10
        {
            let data = sub.receiver.recv().await;
            println!("rx{}: {}", i, data.unwrap());
        }
    });
    let join2 = tokio::task::spawn(async move {
        for _ in 1..10
        {
            datalake2.publish::<String>(&"/test".parse().unwrap(), "hallo".into()).await;
        }
    });
    join1.await.unwrap();
    join2.await.unwrap();
    assert!(false);

}
