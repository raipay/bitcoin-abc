// Copyright (c) 2024 The Bitcoin developers
// Distributed under the MIT software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

import * as React from 'react';
import PropTypes from 'prop-types';
import styled from 'styled-components';
import CopyToClipboard from 'components/Common/CopyToClipboard';
import HideBalanceSwitch from './HideBalanceSwitch';
import { ThemedCopySolid } from 'components/Common/CustomIcons';
import { getWalletsForNewActiveWallet } from 'wallet';
import { Event } from 'components/Common/GoogleAnalytics';

const LabelCtn = styled.div`
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 3%;
    .ant-switch {
        margin-bottom: 5px;
    }
`;

const SELECT_NAME_LENGTH_TO_PX_FACTOR = 15;

const WalletDropdown = styled.select`
    width: ${props =>
        props.value.length * SELECT_NAME_LENGTH_TO_PX_FACTOR > 100
            ? props.value.length * SELECT_NAME_LENGTH_TO_PX_FACTOR
            : 100}px;
    max-width: 90%;
    cursor: pointer;
    font-size: 18px;
    padding: 6px;
    color: ${props => props.theme.contrast};
    border: none;
    border-radius: 9px;
    background-color: transparent;
`;
const WalletOption = styled.option`
    text-align: left;
    background-color: ${props => props.theme.walletInfoContainer};
    :hover {
        color: ${props => props.theme.eCashPurple};
        background-color: ${props => props.theme.walletInfoContainer};
    }
`;

const WalletLabel = ({ wallets, settings, updateCashtabState }) => {
    const address = wallets[0].paths.find(
        pathInfo => pathInfo.path === 1899,
    ).address;

    const handleSelectWallet = e => {
        const walletName = e.target.value;

        // Get the active wallet by name
        const walletToActivate = wallets.find(
            wallet => wallet.name === e.target.value,
        );

        if (typeof walletToActivate === 'undefined') {
            return console.log(`Unable to find wallet ${walletName}`);
        }

        // Get desired wallets array after activating walletToActivate
        const walletsAfterActivation = getWalletsForNewActiveWallet(
            walletToActivate,
            wallets,
        );

        // Event("Category", "Action", "Label")
        // Track number of times a different wallet is activated
        Event('App.js', 'Activate', '');

        // Update wallets to activate this wallet
        updateCashtabState('wallets', walletsAfterActivation);
    };

    return (
        <LabelCtn>
            <WalletDropdown
                name="wallets"
                id="wallets"
                onChange={e => handleSelectWallet(e)}
                value={wallets[0].name}
            >
                {wallets.map((wallet, index) => (
                    <WalletOption key={index} value={wallet.name}>
                        {wallet.name}
                    </WalletOption>
                ))}
            </WalletDropdown>
            <CopyToClipboard data={address} showToast>
                <ThemedCopySolid style={{ marginTop: `8px` }} />
            </CopyToClipboard>
            <HideBalanceSwitch
                settings={settings}
                updateCashtabState={updateCashtabState}
            />
        </LabelCtn>
    );
};

WalletLabel.propTypes = {
    wallets: PropTypes.arrayOf(
        PropTypes.shape({
            mnemonic: PropTypes.string,
            name: PropTypes.string,
            paths: PropTypes.arrayOf(
                PropTypes.shape({
                    address: PropTypes.string,
                    hash: PropTypes.string,
                    path: PropTypes.number,
                    wif: PropTypes.string,
                }),
            ),
            state: PropTypes.shape({
                balanceSats: PropTypes.number,
                nonSlpUtxos: PropTypes.array, // Tx_InNode[]
                slpUtxos: PropTypes.array, // Tx_InNode[]
                tokens: PropTypes.array,
                parsedTxHistory: PropTypes.array,
            }),
        }),
    ),
    settings: PropTypes.oneOfType([
        PropTypes.shape({
            fiatCurrency: PropTypes.string,
            sendModal: PropTypes.bool,
            autoCameraOn: PropTypes.bool,
            hideMessagesFromUnknownSender: PropTypes.bool,
            toggleShowHideBalance: PropTypes.bool,
        }),
        PropTypes.bool,
    ]),
    updateCashtabState: PropTypes.func,
};

export default WalletLabel;
