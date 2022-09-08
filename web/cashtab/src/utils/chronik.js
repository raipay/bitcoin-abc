// Chronik methods
import BigNumber from 'bignumber.js';
/* 
Note: chronik.script('p2pkh', hash160).utxos(); is not readily mockable in jest
Hence it is necessary to keep this out of any functions that require unit testing
*/
export const getUtxosSingleHashChronik = async (chronik, hash160) => {
    // Get utxos at a single address, which chronik takes in as a hash160
    let utxos;
    try {
        utxos = await chronik.script('p2pkh', hash160).utxos();
        if (utxos.length === 0) {
            // Chronik returns an empty array if there are no utxos at this hash160
            return [];
        }
        /* Chronik returns an array of with a single object if there are utxos at this hash 160
        [
            {
                outputScript: <hash160>,
                utxos:[{utxo}, {utxo}, ..., {utxo}]
            }
        ]
        */

        // Return only the array of utxos at this address
        return utxos[0].utxos;
    } catch (err) {
        console.log(`Error in chronik.utxos(${hash160})`);
        console.log(err);
    }
};

export const returnGetUtxosChronikPromise = (chronik, hash160AndAddressObj) => {
    /*
        Chronik thinks in hash160s, but people and wallets think in addresses
        Add the address to each utxo
    */
    return new Promise((resolve, reject) => {
        getUtxosSingleHashChronik(chronik, hash160AndAddressObj.hash160).then(
            result => {
                for (let i = 0; i < result.length; i += 1) {
                    const thisUtxo = result[i];
                    thisUtxo.address = hash160AndAddressObj.address;
                }
                resolve(result);
            },
            err => {
                reject(err);
            },
        );
    });
};

export const getUtxosChronik = async (chronik, hash160sMappedToAddresses) => {
    /* 
        Chronik only accepts utxo requests for one address at a time
        Construct an array of promises for each address
        Note: Chronik requires the hash160 of an address for this request
    */
    const chronikUtxoPromises = [];
    for (let i = 0; i < hash160sMappedToAddresses.length; i += 1) {
        const thisPromise = returnGetUtxosChronikPromise(
            chronik,
            hash160sMappedToAddresses[i],
        );
        chronikUtxoPromises.push(thisPromise);
    }
    const allUtxos = await Promise.all(chronikUtxoPromises);
    // Since each individual utxo has address information, no need to keep them in distinct arrays
    // Combine into one array of all utxos
    const flatUtxos = allUtxos.flat();
    return flatUtxos;
};

export const organizeUtxosByType = chronikUtxos => {
    /* 
    
    Convert chronik utxos (returned by getUtxosChronik function, above) to match 
    shape of existing slpBalancesAndUtxos object
    
    This means sequestering eToken utxos from non-eToken utxos

    For legacy reasons, the term "SLP" is still sometimes used to describe an eToken

    So, SLP utxos === eToken utxos, it's just a semantics difference here
    
    */

    const nonSlpUtxos = [];
    const slpUtxos = [];
    for (let i = 0; i < chronikUtxos.length; i += 1) {
        // Construct nonSlpUtxos and slpUtxos arrays
        const thisUtxo = chronikUtxos[i];
        if (typeof thisUtxo.slpToken !== 'undefined') {
            slpUtxos.push(thisUtxo);
        } else {
            nonSlpUtxos.push(thisUtxo);
        }
    }

    return { slpUtxos, nonSlpUtxos };
};

export const getPreliminaryTokensArray = slpUtxos => {
    // Iterate over the slpUtxos to create the 'tokens' object
    let tokensById = {};

    slpUtxos.forEach(slpUtxo => {
        /* 
        Note that a wallet could have many eToken utxos all belonging to the same eToken
        For example, a user could have 100 of a certain eToken, but this is composed of
        four utxos, one for 17, one for 50, one for 30, one for 3        
        */

        // Start with the existing object for this particular token, if it exists
        let token = tokensById[slpUtxo.slpMeta.tokenId];

        if (token) {
            if (slpUtxo.slpToken.amount) {
                token.balance = token.balance.plus(
                    new BigNumber(slpUtxo.slpToken.amount),
                );
            }
        } else {
            // If it does not exist, create it
            token = {};
            token.tokenId = slpUtxo.slpMeta.tokenId;
            if (slpUtxo.slpToken.amount) {
                token.balance = new BigNumber(slpUtxo.slpToken.amount);
            } else {
                token.balance = new BigNumber(0);
            }
            tokensById[slpUtxo.slpMeta.tokenId] = token;
        }
    });

    const preliminaryTokensArray = Object.values(tokensById);
    return preliminaryTokensArray;
};

const returnGetTokenInfoChronikPromise = (chronik, tokenId) => {
    /*
    The chronik.tx(txid) API call returns extensive transaction information
    For the purposes of finalizing token information, we only need the token metadata

    This function returns a promise that extracts only this needed information from
    the chronik.tx(txid) API call

    In this way, calling Promise.all() on an array of tokenIds that lack metadata
    will return an array with all required metadata
    */
    return new Promise((resolve, reject) => {
        chronik.tx(tokenId).then(
            result => {
                if (typeof result === 'undefined') {
                    console.log(`result`, result);
                }

                const thisTokenInfo = result.slpTxData.genesisInfo;
                thisTokenInfo.tokenId = tokenId;
                // You only want the genesis info for tokenId
                resolve(thisTokenInfo);
            },
            err => {
                reject(err);
            },
        );
    });
};

export const processPreliminaryTokensArray = (
    preliminaryTokensArray,
    tokenInfoByTokenId,
) => {
    /* Iterate over preliminaryTokensArray to

    1 - Add slp metadata (token ticker, name, other metadata)
    2 - Calculate the token balance. Token balance in 
        preliminaryTokensArray does not take into account the
        decimal places of the token...so it is incorrect.

    */
    const finalTokenArray = [];
    for (let i = 0; i < preliminaryTokensArray.length; i += 1) {
        const thisToken = preliminaryTokensArray[i];
        const thisTokenId = thisToken.tokenId;

        // Because tokenInfoByTokenId is indexed by tokenId, it's easy to reference
        const thisTokenInfo = tokenInfoByTokenId[thisTokenId];

        // The decimals are specifically needed to calculate the correct balance
        const thisTokenDecimals = thisTokenInfo.decimals;

        // Add info object to token
        thisToken.info = thisTokenInfo;

        // Update balance according to decimals
        thisToken.balance = thisToken.balance.shiftedBy(-1 * thisTokenDecimals);

        // Now that you have the metadata and the correct balance,
        // preliminaryTokenInfo is finalTokenInfo
        finalTokenArray.push(thisToken);
    }
    return finalTokenArray;
};

export const finalizeTokensArray = async (chronik, preliminaryTokensArray) => {
    // Iterate over preliminaryTokensArray to determine what tokens you need to make API calls for

    // Create an array of promises
    // Each promise is a chronik API call to obtain token metadata for this token ID
    const getTokenInfoPromises = [];

    for (let i = 0; i < preliminaryTokensArray.length; i += 1) {
        const thisTokenId = preliminaryTokensArray[i].tokenId;
        const thisTokenInfoPromise = returnGetTokenInfoChronikPromise(
            chronik,
            thisTokenId,
        );
        getTokenInfoPromises.push(thisTokenInfoPromise);
    }

    // Get all the token info you need
    let tokenInfoArray = [];
    try {
        tokenInfoArray = await Promise.all(getTokenInfoPromises);
    } catch (err) {
        console.log(`Error in Promise.all(getTokenInfoPromises)`, err);
    }

    // Create a reference object that indexes token metadata by token ID
    // This object can then be used to add token metadata to each token ID
    const tokenInfoByTokenId = {};
    for (let i = 0; i < tokenInfoArray.length; i += 1) {
        /* tokenInfoArray is an array of objects that look like
        {
            "tokenTicker": "ST",
            "tokenName": "ST",
            "tokenDocumentUrl": "developer.bitcoin.com",
            "tokenDocumentHash": "",
            "decimals": 0,
            "tokenId": "bf24d955f59351e738ecd905966606a6837e478e1982943d724eab10caad82fd"
        }
        */

        const thisTokenInfo = tokenInfoArray[i];
        const thisTokenId = thisTokenInfo.tokenId;
        // Add this entry to updatedCachedTokenInfo
        tokenInfoByTokenId[thisTokenId] = thisTokenInfo;
    }

    // Now use tokenInfoByTokenId object to finalize token info
    // Split this out into a separate function so you can unit test
    const finalTokenArray = processPreliminaryTokensArray(
        preliminaryTokensArray,
        tokenInfoByTokenId,
    );

    return finalTokenArray;
};