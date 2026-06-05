import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"

import "@wrappers/EnWalletV5.gen"
import "@examples/l2-wallet-v5/storage"

const ENWALLET_TEST_WALLET_ID = 0x7ffffffd
const ENWALLET_TEST_PUBLIC_KEY = 0x111122223333444455556666777788889999aaaabbbbccccddddeeeeffff0000

get fun `test enwallet v5 deploy initializes wallet state`() {
    val deployer = testing.treasury("enwallet-v5-deployer");
    val wallet = EnWalletV5.fromStorage({
        isSignatureAllowed: true,
        seqno: 0,
        subwalletId: ENWALLET_TEST_WALLET_ID,
        publicKey: ENWALLET_TEST_PUBLIC_KEY,
        extensions: ([] as ExtensionsDict),
    });

    val deploy = wallet.deploy(deployer.address, { value: ton("0.1") });
    expect(deploy).toHaveSuccessfulDeploy({
        from: deployer.address,
        to: wallet.address,
    });

    expect(wallet.isSignatureAllowed()).toEqual(true);
    expect(wallet.seqno()).toEqual(0);
    expect(wallet.getWalletId()).toEqual(ENWALLET_TEST_WALLET_ID);
    expect(wallet.getSubwalletId()).toEqual(ENWALLET_TEST_WALLET_ID);
    expect(wallet.getPublicKey()).toEqual(ENWALLET_TEST_PUBLIC_KEY);
}
