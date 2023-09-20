use std::error::Error;
use std::future::Future;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use futures::FutureExt;
use types::MessageType;

mod types;

const BUF_SIZE: usize = 65535;

async fn copy_with_abort<R, W>(read: &mut R, write: &mut W, mut abort: broadcast::Receiver<()>, print_data: bool) -> tokio::io::Result<usize>
    where R: tokio::io::AsyncRead + Unpin, W: tokio::io::AsyncWrite + Unpin {
    let mut bytes_forwarded = 0;
    let mut buf = [0u8; BUF_SIZE];

    loop {
        let bytes_read;
        tokio::select! {
                biased;

                result = read.read(&mut buf) => {
                    use std::io::ErrorKind::{ConnectionReset, ConnectionAborted};
                    bytes_read = result.or_else(|e| match e.kind() {
                        ConnectionReset | ConnectionAborted => Ok(0),
                        _ => Err(e)
                    })?;
                },
                _ = abort.recv() => {
                    break;
                }
            }

        if bytes_read == 0 {
            break;
        }

        if print_data {
            println!("Read {bytes_read} bytes");
            println!("{}", &buf[0..bytes_read].iter().map(|b| format!("{:02x}", b)).collect::<String>());
        }

        write.write_all(&buf[0..bytes_read]).await?;
        bytes_forwarded += bytes_read;
    }

    Ok(bytes_forwarded)
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // https://github.com/mqudsi/tcpproxy/blob/master/src/main.rs
    let listener = TcpListener::bind("0.0.0.0:5279").await?;
    println!("Listening on {}", listener.local_addr().unwrap());

    loop {
        let (client, client_addr) = listener.accept().await?;
        tokio::spawn(handle_connection(client, client_addr));
    }
}

async fn handle_connection(mut client_stream: TcpStream, client_addr: SocketAddr) -> () {
    println!("New connection from {}", client_addr);

    let mut remote_server = match TcpStream::connect("server.growatt.com:5279").await {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Error establishing connection: {e}");
            return;
        }
    };

    let (mut client_read, mut client_write) = client_stream.split();
    let (mut remote_read, mut remote_write) = remote_server.split();

    let (cancel, _) = broadcast::channel(1);

    let (remote_copied, client_copied) = tokio::join! {
                copy_with_abort(&mut remote_read, &mut client_write, cancel.subscribe(), false).then(|r| {
                    let _ = cancel.send(()); async {r}
                }),
                copy_with_abort(&mut client_read, &mut remote_write, cancel.subscribe(), true).then(|r| {
                    let _ = cancel.send(()); async {r}
                })
            };

    match client_copied {
        Ok(count) => {
            eprintln!(
                "Transferred {} bytes from proxy client {} to upstream server",
                count, client_addr
            );
        }
        Err(err) => {
            eprintln!(
                "Error writing bytes from proxy client {} to upstream server",
                client_addr
            );
            eprintln!("{}", err);
        }
    };

    match remote_copied {
        Ok(count) => {
            eprintln!(
                "Transferred {} bytes from upstream server to proxy client {}",
                count, client_addr
            );
        }
        Err(err) => {
            eprintln!(
                "Error writing bytes from upstream server to proxy client {}",
                client_addr
            );
            eprintln!("{}", err);
        }
    };
}

fn old_main() -> Result<(), String> {
    enum _InverterType {
        Default,
        Sph,
        Spf,
        Max,
    }

    // SPH 1000TL3 BH-UP

    // let data = "000100060124221b032a2b4023394077415c7761747447726f7761747447726f7761747447726f34767d66467e42779b45437242585959455875435c434f44406b5f5e43515a406b5f57474f405876435f5952585975405c59565859764b5659555859744b5859595846714641464d4645735c5b5b5545456943435a515a416b5f5f595258446940434552475a735e5d44515a4c6b405c444f4658765c5d5b505a446b43414e4d5945764a414e4d594c754341474d594c754341474d594469434343585a4d6b4156464f465876455e46575a416b475c4f4f4758764756424f425876455944565a476b475f4f4f465872465b59585845774a5f59595841774a41454d4140735c565b50444c775c575b504d427343414f545840774041474d4044745c585b55444669404369e2";
    // let data = "00260006033f0204032a2b4023394077415c7761747447726f7761747447726f7761747447723b272b46360a335f4350747447726f7761747447726f7761747447726f77767d6656735f746174743b726a7761793c480d6f736174723e7cc77765747440796f76e1f17447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f753f67fb480a6f716174665562617767747447727f6961727447726f7761747447726f776132744606f575f1cd9847726f556174bc68726f7744747493d56f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726eebb775bc46bf6eb0617466e16e3d7761747447726f7761747447726f7761747447726f7761747447726f776c7474546d6f7761747447726e7761747447726f77617477af760b7764747447726f7761747447726f7761747447726f7761747fff60f1775a747447726f7761747447726f7761747447726f776179745c72797761747447726f7760e474477262777a746247726ee76174756f726a6b28748e47726f7761747447726f7761747447726f77614c7447599d7761744e47722be16174740c726efe77747447726f776174744a726f77617474476a6f776174744772627761747447726f776174744772eb7761747447497de9615c75c172957c4366d847726f7a614a742361977761747447726f7761747447726d7761747447727c8f71ec78bd7e82700d7575465a6e7360767d6672ff7768747447726f7761747447760a7380747947727c686174744771877761747447726f7761747447726f772a7475ce646f77613f7446fb79776175e447726ee761747400726e7763387d47736f752afb65b86390669e658b47726f7761747447726f7761747447726fe661747447726f7761747447726f7761747447726f7761747447726f7761747447726f77617474471d6f7a616f74517279777d73a40952936fb47c7446726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726ff717";
    // let data = "002c000600200116032a2b4023394077415c7761747447726f7761747447726f77617474477228d5";
    // let data = "001600060003010447c698";
    // let data = "001b0006033f0204032a2b4023394077415c7761747447726f7761747447726f7761747447723b272b46360a335f4350747447726f7761747447726f7761747447726f77767d66557975746174743b726a776174744ab06f77617474477f1a7761747447726f76e1f17447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f613267fc48bc6f7d617468e57dc6776b7474477260b8617d7447726f7761747447726f77613f744606f075f1aefb47726f546174bc77726f7747747493da6f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726eebb975b746b26ecd6174663369be7761747447726f7761747447726f7761747447726f7761747447726f776c7474546d6f7761747447726e7761747447726f77617477af760b7764747447726f7761747447726f7761747447727b8d6174744760017758747447726f7761747447726f7761747447726f7761c6748f72db7761747447726f7775c0744772dd77a974c047727bc361747561726a6ca9748e47726f7761747447726f7761747447726f776149744759987761744e47722be161747417726efe7a747447726f7761747448726f7761747447696f776174744772627761747447726f776174744772ed77617474474b7d199e0a75c372957dcb66d847726f7d614a742361977761747447726f7761747447726d7761747447727c8f71ec78947ea67e0c752146546e7560767d6672ff7768747447726f7761747447760a7380747947727c686174744771877761747447726f7761747447726f77317475ce696f7761247446fb74776160c047727bc36174740e726e7763383547736f752ae465b86390669e658b47726f7761747447726f7761747447726fe661747447726f7761747447726f7761747447726f7761747447726f7761747447726f77617474471d6fc461bd74f272da777d73a40952936fb47c7446726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f03f8";
    // let data = "000100060120221b032a2b4023394077415c7761747447726f7761747447726f7761747447726f34767d665574757797454372435859514c5875435b434f40406b5f57424f40586a4457595858457f4a41444d59457f4341414d5945744041404d5947754a414f4d4644765c5e5b5041446946434456445a705e42474f40586a4241434d445a735e5d44595a446b405d4e4f445875415d59555845694343474f4d58765c565b515a446b5f5943525a456b5f5943525a456b5f5f59515841775c5f5b524d47694a4346564546745c5d5b54474c6945434657444169444346564240705c575b5445446946434255435a735e5e4759425a725e5a46515a406b475b404f4058764257414f4158764b5941505a41755e5b47525a436b465f434f455873425c5950583448";
    // let data = "000100060121221b032a2b4023394077415c7761747447726f7761747447726f7761747447726f34767d6655677d7796454372435859504c5875435b434f42446b5f56414f46586a4659595258457f4a41474d59457040414e4d5945734241464d5947754041464d454d705c595b50404269454344574c5a755e42474f40586a4241454d445a725e5d44555a436b405c444f4c5875415f59565845694243474f4d58765c565b57465a745e424152415a765e424152415a765e42474f4458724241474d474d745c575b504345754141444d41477f5c585b504244725c575b5043427345414e4d4145775c5b5b54404369474346514c426944434250445a735e5a43565a416b435f4f575a426b43564157455a704b434351475a725e5b47555a436b465f454f4658b4e3";
    // let data = "00010006011e221b032a2b4023394077415c7761747447726f7761747447726f7761747447726f34767d665556657795454372435859534c5875435b434f43436b455e59595845754741454d474d755c5c5b4c46477e5c5f5b4c46457e5c585b4c4045755c5d5b53404d6945434554445a7f5e5a42565a466b4241454d445a735e5f59575846754b41454d4647725c565b534741694243464f4558765c5d5b535a416b47594f4f4d586a4a58444f42586a4a58444f4258775c5a5b554d5a7e5e5c4e525a4d6b43584653475a745e5a44595a436b435947575a446b435841554c5a775e5a46515a416b475b404f4158764257414f425872435f59545841734541424d45447f4441414d454d71445d5951415873425c59515840774741424d4044745c5c5b012d";

    // richiesta dal server
    let messages = [
        "0001000600250119032a2b4023394077415c7761747447726f7761747447726f7761747447726f7361754150ac",
        "0001000600240119032a2b4023394077415c7761747447726f7761747447726f7761747447726f736161339d",
        "00010006002e0119032a2b4023394077415c7761747447726f7761747447726f7761747447726f7f617e2c1f2a372f392c2c1f2af84d",
        "0001000600270119032a2b4023394077415c7761747447726f7761747447726f7761747447726f7e6177466942f43b",
        "0001000600250119032a2b4023394077415c7761747447726f7761747447726f7761747447726f7d617544bb6e",
        "00010006002b0119032a2b4023394077415c7761747447726f7761747447726f7761747447726f62617345694241424f4dabf6",
        "0001000600260119032a2b4023394077415c7761747447726f7761747447726f7761747447726f7a61764575efa4",
        "0001000600240119032a2b4023394077415c7761747447726f7761747447726f7761747447726f68616b336d",
        "0001000600250119032a2b4023394077415c7761747447726f7761747447726f7761747447726f35617546daf8",
        "0001000600250119032a2b4023394077415c7761747447726f7761747447726f7761747447726f34617544e778",
        "0001000600270119032a2b4023394077415c7761747447726f7761747447726f7761747447726f286177456942c2c7",
        "0001000600250119032a2b4023394077415c7761747447726f7761747447726f7761747447726f166175449f72",
        "0001000600370119032a2b4023394077415c7761747447726f7761747447726f7761747447726f6861674677405c5a514d59764a4f46584e4776485a465f4d",
        "000100060123221b032a2b4023394077415c7761747447726f7761747447726f7761747447726f34767d66546d5a7798454372435759637474470e6f76c93c7447720b77055364477248676f642d06434147612e3006336f7f61757447726f777f755847d26fd761742017385d352c354473436e7499d0744572357761747467524f39040354021c0a05060d5467526f766f65744775887768746647616f68615974467f276651670054ee6d2f708c66c966717761747447726f776c4e6559611164f3743f47e46f7d617e7442726a7764747147726f7761747447637177612d3606335f445147447f726f7650747447727ceb616c655962a179607a2547666f6371237bd772777779746c476a6f6f616c7444726f7685748b09526f882f5474b83c4f779e3a544671a951",
        "0001000600250119032a2b4023394077415c7761747447726f7761747447726f7761747447726f7361754150ac",
        "0001000600250119032a2b4023394077415c7761747447726f7761747447726f7761747447726f726175456fac",
        "0001000600260119032a2b4023394077415c7761747447726f7761747447726f7761747447726f71617647754e00",
        "00010006002e0119032a2b4023394077415c7761747447726f7761747447726f7761747447726f7f617e2c1f2a372f392c2c1f2af84d",
        "000100060124221b032a2b4023394077415c7761747447726f7761747447726f7761747447726f34767d66467e42779b45437242585959455875435c434f44406b5f5e43515a406b5f57474f405876435f5952585975405c59565859764b5659555859744b5859595846714641464d4645735c5b5b5545456943435a515a416b5f5f595258446940434552475a735e5d44515a4c6b405c444f4658765c5d5b505a446b43414e4d5945764a414e4d594c754341474d594c754341474d594469434343585a4d6b4156464f465876455e46575a416b475c4f4f4758764756424f425876455944565a476b475f4f4f465872465b59585845774a5f59595841774a41454d4140735c565b50444c775c575b504d427343414f545840774041474d4044745c585b55444669404369e2",
        "00860006033f0204032a2b4023394077415c7761747447726f7761747447726f7761747447723b272b46360a335f4350747447726f7761747447726f7761747447726f77767d66467e41746174743b72697761747447726f7761747447726f7761747447726f76e1f17447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f652067f248c36f7e61746d497dab77687474477260c1617c7447726f7761747447726f7761717446063675ef897847726f776174bc4a726f7761747493f06f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726eebee75d846e86ed76174663669dd7761747447726f7761747447726f7761747447726f7761747447726f7760747454616f7761747447726e7761747447726f77617477af760b7767747447726f7761747467726f7761747447727a6f617474476003775e747447726f7761747447726f7761747447726f7761fa74ef72fb7761747447726f77746c744772e177c974e047727a6f6174756f72696cc4748e47726f7761747447726f7761747447726f776173744759ae7761747447722b2b61747440726effb3747447726f7761747449726f7761747447686f776174744772617761747447726f776174744772ed77617474474d7d1b9e0075c672957cad66d847726f626149742361977761747447726f7761747447726d7761747447727c8f71ec789a7ea7700d7225465a6e7660767d6672ff7768747447726f7761747447760a7380747547727c646174744771877761747447726f7761747447726f77667475cfa06f7761737446fabd7761616c47727a6f61747447726e77633d8947736f7528b665b86390669e658b47726f7761747447726f7761747447726fe661747447726f7761747447726f7761747447726f7761747447726f7761747447726f77617474471d6ff861dc74d372fb777d73a40952936fb47c7446726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f8c76",
        "000100060123221b032a2b4023394077415c7761747447726f7761747447726f7761747447726f34767d66466342779845437242585959475875435c434f45446b5f5e43535a406b5f584f4f445876435a5955585975405b59555859764b5659535859744b58595758467147414f4d4645735c595b5545416941435a515a416b5f5f595258446940434552475a735e5d44515a436b405c454f4058765c5d5b505a446b4041474d594c705c565b4c4c46765c5d5b4c4c46765c5d5b4c445a765e5b4e4f4d58744b5e5953584570435e414f415872415759525845724b5a5956584570445c404f4058724257595258417346414e4d45447f42414f4d41447f5c5c5b544040694b4346514c44694a4346584240765c56434d4044765c5b5b554446694b434351455a725e1b81",
        "008d0006033f0204032a2b4023394077415c7761747447726f7761747447726f7761747447723b272b46360a335f4350747447726f7761747447726f7761747447726f77767d66466343746174743b72697761747447726f7761747447726f7761747447726f76e1f17447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f654467f348de6f7f61746d277dd977687474477260c5617c7447726f7761747447726f7761727446063575ef8b1547726f776174bc4a726f7761747493f06f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726eebee75d846e86ee86174663669f37761747447726f7761747447726f7761747447726f7761747447726f7760747454616f7761747447726e7761747447726f77617477af760b7767747447726f7761747467726f7761747447727a0b617474476003775e747447726f7761747447726f7761747447726f7761e474ed72f97761747447726f777408744772ff77cb74e247727a0b6174756f72696cf3748e47726f7761747447726f7761747447726f776173744759ae7761747447722b2b61747440726effb3747447726f7761747448726f7761747447696f776174744772627761747447726f776174744772ed77617474474d7d1b9e0075c772957ca366d847726f626149742361977761747447726f7761747447726d7761747447727c8f71ec789b7ea8700d7721465a6e7760767d6672ff7768747447726f7761747447760a7380747547727c646174744771877761747447726f7761747447726f77697475cfa16f77617c7446fabc7761610847727a0b61747447726e77633e7647736f7528b665b86390669e658b47726f7761747447726f7761747447726fe661747447726f7761747447726f7761747447726f7761747447726f7761747447726f77617474471d6fe761de74d172f9777d73a40952936fb47c7446726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726fafd2",
        "00360006033f0104032a2b4023394077415c7761747447726f7761747447726f7761747447723b272b46360a335f4620747447726f7761747447726f7761747447726f77767d6657625a746174743b726a7761609749986f7061747e937c10776674744d496f76e1f17447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f65ac67f248076f7e61746df3626f7768747447727f41617e7447726f7761747447726f77613c7446f84675f1e90747726f506174a09c726f7744747490996f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726edba775d546d06efd617466216e0a7761747447726f7761747447726f7761747447726f7761747447726f776074744bd26f7761747447726e7761747447726f77617477af760b7764747447726f7760747447726f7761707447726f776174756b6008775e747447726f7761747447726f7761747447726f7761d474d572de7761747447726f7772fc744772cf77f374c547727cff6174755d726a6b13748e47726f7761747447726f7761747447726f77617474476f917761747747725c536174740c726eed5d747447726f7761747441726f7761747447626f7761747447727a7761747447726f776174744772eb77615470474d7d10617475c672957b3967fc47726f476159742361977761747447726f7761747447726d7761745447727c8f71ec78887ef07f1c762746686f8e60767d6572ff7768747447726f7761747447760a73807475477263d76174744771877761747447726f7761747447726f772b7475e7d06f77613e7446d2cd7761679847727c9b6174740b726e7760e16847736f76d11e65b86390669e658b47726f7761747447726f7761747447726fe661747447726f7761747447726f7761747447726f7761747447726f7761747447726f77617474471d6fd661e574f672de777d73a40952936fb47c7446726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f7761747447726f5f59"
    ];

    let mut data_bytes = Vec::new();

    for message in messages {
        // Data is a string with decimal values of hex characters. Convert it into a vector of bytes.
        let mut bytes = Vec::new();
        for i in 0..message.len() / 2 {
            let byte = u8::from_str_radix(&message[2 * i..2 * i + 2], 16).unwrap();
            bytes.push(byte);
        }
        data_bytes.push(bytes);
    }

    // let payload_length = bytes.len();
    for (index, bytes) in data_bytes.iter().map(|bytes| decrypt(bytes)).enumerate() {
        println!("\nAnalizing #{}", index + 1);
        // println!("Decrypted data:");
        // print_bytes(&bytes, 16);

        // println!("\n\n{}", hex_bytes_to_ascii(&dec));


        let data_length = u16::from_be_bytes(bytes[4..6].try_into().unwrap());

        println!("Data length: {data_length} bytes");

        let message_type = match bytes[7] {
            0x03 => MessageType::DATA3,
            0x04 => MessageType::DATA4,
            0x16 => MessageType::PING,
            0x18 => MessageType::CONFIGURE,
            0x19 => MessageType::IDENTIFY,
            v => MessageType::UNKNOWN(v),
        };
        println!("Message type: {:?}", message_type);

        let mut pos = 8usize;

        let datalogger_id = hex_bytes_to_ascii(&bytes[pos..pos + 30]);
        println!("Datalogger serial: '{datalogger_id}'");

        if !matches!(message_type, MessageType::IDENTIFY) {
            println!("Message is not an identification. Skipping for now...");

            if let MessageType::UNKNOWN(t) = message_type {
                println!("!!!UNKNOWN MESSAGE TYPE: 0x{:02x}", t);
            }

            print_bytes(&bytes, 16);

            continue;
        }

        pos += 30;

        let detail_subtype = u16::from_be_bytes(bytes[pos..pos + 2].try_into().unwrap());
        println!("Message subtype: {} (0x{:02x})", detail_subtype, detail_subtype);

        if [0x04, 0x1f].contains(&detail_subtype) {
            println!("Unknown message subtype... Skipping for now...");
            continue;
        }

        pos += 2;

        let subtype_length = u16::from_be_bytes(bytes[pos..pos + 2].try_into().unwrap());
        println!("Subtype data length: {data_length}");

        pos += 2;

        let data = hex_bytes_to_ascii(&bytes[pos..pos + subtype_length as usize]);
        println!("Data: '{data}'");
    }

    Ok(())
}

fn decrypt(decdata: &[u8]) -> Vec<u8> {
    let ndecdata = decdata.len();
    let mask = b"Growatt";

    // Start the decrypt routine
    let mut unscrambled: Vec<u8> = decdata[..8].to_vec(); // Take the unscramble header

    for (i, j) in (8..ndecdata).zip((0..).cycle().take(ndecdata - 8)) {
        let xor_value = decdata[i] ^ mask[j % mask.len()];
        unscrambled.push(xor_value);
    }

    unscrambled
}

fn hex_bytes_to_ascii(hex_bytes: &[u8]) -> String {
    hex_bytes.iter().map(|b| b.clone() as char).collect()
}

fn print_bytes(bytes: &[u8], n: usize) {
    bytes.chunks(n).enumerate().for_each(|(i, chunk)| {
        if i != 0 {
            println!();
        }
        print!("{:04x}: ", i * n);
        chunk.iter().enumerate().for_each(|(j, byte)| {
            if j != 0 && j % (n / 2) == 0 {
                print!(" ");
            }
            print!("{:02x} ", byte);
        });
        print!("  ");
        chunk.iter().for_each(|byte| {
            print!("{}", *byte as char);
        });
    });
}
