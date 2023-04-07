pub mod data_lake;
use data_lake::*;
//use tokio_stream::stream_ext::StreamExt;
//use bus::MessagePackBusAccess;
//use bus::ConnectionSetup;


//use futures::stream::Stream;


mod BusAccess
{
    use regex::Regex;
    pub mod bus {
        tonic::include_proto!("_");
    }
    use crate::data_lake::*;
    use tokio_stream::StreamExt;

    pub struct BusAccessHandle
    {
        stop_sender: Option<tokio::sync::oneshot::Sender<()>>,
        join_handle: Option<tokio::task::JoinHandle::<()>>,
    }

    // TODO: use `signal-hook` crate to catch signals and cleanly exit the main() function in order
    // to utilize drop here...
    impl Drop for BusAccessHandle
    {
        fn drop(self: &mut Self)
        {
            println!("DROP!!");
            self.stop_sender.take().unwrap().send(());
            tokio::runtime::Handle::try_current().unwrap().block_on(self.join_handle.take().unwrap());
        }
    }

    pub fn create(datalake: TDataLake, server_url: String, datalake_base_path: String) -> BusAccessHandle
    {
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        let join_handle = tokio::task::spawn(
            receive_from_bus_and_publish(datalake, server_url, datalake_base_path, rx)
        );

        BusAccessHandle{stop_sender: Some(tx), join_handle: Some(join_handle)}
    }
    async fn receive_from_bus_and_publish(datalake: TDataLake, server_url: String, publish_base_path: String, mut stop_receiver: tokio::sync::oneshot::Receiver<()>)
    {
        let mut client = bus::message_pack_bus_access_client::MessagePackBusAccessClient::connect(server_url).await.unwrap();

        let req = bus::ConnectionSetup {
            rx_mask: "0:255".into(),
            remote_address: "0:1".into(),
        };
        // receive from bus (data to send to data lake):
        let response = client.receive(req).await.unwrap();
        let mut resp_stream = response.into_inner();

        // receive from lake (data to send to bus)
        let fisher = datalake.subscribe("/bus/tx".parse().unwrap())
        
        loop
        {
            tokio::select!
            {
                grpc_event = resp_stream.next() =>
                {
                    if let Some(received) = grpc_event
                    {
                        let received = received.unwrap();
                        println!("\treceived message: `{:?}`", received);

                        let re = Regex::new(r".*([0-9]+):.*").unwrap();
                        let captures = re.captures(received.remote_address.as_str());
                        if let Some(captures) = captures 
                        {
                            if let Some(deviceId) = captures.get(1)
                            {
                                datalake.publish(&("/bus/rx/".to_owned() + deviceId.as_str()).as_str().parse().unwrap(), received.data).await;
                            }
                        }
                    }
                    else 
                    {
                        // TODO reconnect here
                        break;
                    }
                }
                transmit_request = fisher.receive() =>
                {
                    let req = bus::SendJsonMessageRequest {
                        remote_address: transmit_request.device_id + ":1",
                        data: transmit_request.json_payload,
                        timeout_milliseconds: 5000
                    };
                    let result = client.send(req).await;
                    // TODO: this will block the select?
                    // handle result
                }
                _ = &mut stop_receiver =>
                {
                    // Quit
                    break;
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> ()
{
    let mut datalake = TDataLake::new();

    let bus_handle = BusAccess::create(datalake.clone(), "http://192.168.0.200:50051".into(), "bus/receive/ug".into());


    let mut sub = datalake.subscribe::<String>(&"/bus/rx/*".parse().unwrap()).await;
    loop
    {
        tokio::select!
        {
            data = sub.receiver.recv() =>
            {
                println!("rx: {}", data.unwrap());
            }
            s = tokio::signal::ctrl_c() =>
            {
                s.unwrap();
                println!("ctrl-c received!");
                break;
            }
        }
    }

    //bus_handle.stop();

    //join1.await.unwrap();
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
