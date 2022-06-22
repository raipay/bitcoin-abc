use bitcoinsuite_core::Script;
use bitcoinsuite_error::Result;
use bitcoinsuite_test_utils_blockchain::{setup_bch_chain, setup_xec_chain};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_setup_xec_chain() -> Result<()> {
    let redeem_script = Script::from_slice(&[0x51]);
    let (bitcoind, utxos) = setup_xec_chain(10, &redeem_script).await?;
    assert_eq!(bitcoind.cmd_string("getblockcount", &[])?, "110");
    assert_eq!(utxos.len(), 10);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_setup_bch_chain() -> Result<()> {
    let redeem_script = Script::from_slice(&[0x51]);
    let (bitcoind, utxos) = setup_bch_chain(10, &redeem_script).await?;
    assert_eq!(bitcoind.cmd_string("getblockcount", &[])?, "110");
    assert_eq!(utxos.len(), 10);
    Ok(())
}
