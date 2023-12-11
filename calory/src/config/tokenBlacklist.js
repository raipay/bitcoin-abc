// Copyright (c) 2023 The Bitcoin developers
// Distributed under the MIT software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

'use strict';
import {
    cashtabSettingsValidation,
    supportedFiatCurrencies,
} from 'config/cashtabSettings';

// Get names of fiat currencies
const fiatObjects = Object.keys(supportedFiatCurrencies);
let fiatNames = [];
let fiatSymbols = [];
for (let i in fiatObjects) {
    fiatNames.push(supportedFiatCurrencies[fiatObjects[i]].name);
    fiatSymbols.push(supportedFiatCurrencies[fiatObjects[i]].symbol);
}

const tokenBlacklist = {
    coingeckoTop500Tickers: [
        'btc',
        'eth',
        'usdt',
        'bnb',
        'usdc',
        'xrp',
        'busd',
        'ada',
        'sol',
        'doge',
        'dot',
        'matic',
        'steth',
        'shib',
        'dai',
        'trx',
        'wbtc',
        'avax',
        'uni',
        'okb',
        'leo',
        'ltc',
        'atom',
        'link',
        'etc',
        'ftt',
        'xlm',
        'cro',
        'qnt',
        'xmr',
        'near',
        'algo',
        'bch',
        'vet',
        'lunc',
        'flow',
        'fil',
        'hbar',
        'ape',
        'egld',
        'wbt',
        'frax',
        'icp',
        'apt',
        'aave',
        'xtz',
        'xcn',
        'ht',
        'tkx',
        'sand',
        'mana',
        'ldo',
        'eos',
        'theta',
        'chz',
        'cusdc',
        'kcs',
        'bsv',
        'usdp',
        'axs',
        'tusd',
        'mkr',
        'evmos',
        'btt',
        'usdd',
        'xec',
        'miota',
        'ethw',
        'klay',
        'zec',
        'cdai',
        'gt',
        'cake',
        'grt',
        'btse',
        'neo',
        'osmo',
        'ceth',
        'hnt',
        'snx',
        'nexo',
        'ftm',
        'ar',
        'xrd',
        'paxg',
        'cspr',
        'crv',
        'kava',
        'bit',
        'ens',
        'twt',
        'dash',
        'rune',
        'zil',
        'rpl',
        'xdc',
        'enj',
        'bat',
        'xaut',
        'stx',
        'fxs',
        'cel',
        'mina',
        'dcr',
        'dfi',
        'amp',
        'luna',
        'rvn',
        'ustc',
        'cusdt',
        'cvx',
        '1inch',
        'mex',
        'comp',
        'omi',
        'hot',
        'xem',
        'gusd',
        'celo',
        'waves',
        'imx',
        'btg',
        'lrc',
        'ksm',
        'rose',
        'nxm',
        'tfuel',
        'gmx',
        'gno',
        'ohm',
        'gmt',
        'qtum',
        'sushi',
        'safemoon',
        'okt',
        'srm',
        'iost',
        'glm',
        'jst',
        'hbtc',
        'gala',
        'kda',
        'cvxcrv',
        'iotx',
        'rsr',
        'yfi',
        'poly',
        'elg',
        'lpt',
        'ankr',
        'bal',
        'omg',
        'bone',
        'msol',
        'syn',
        'zrx',
        'mc',
        'one',
        'reth',
        'eurt',
        'dydx',
        'mim',
        'dag',
        'flux',
        'icx',
        'glmr',
        'woo',
        'juno',
        'rbn',
        'ont',
        'babydoge',
        'op',
        'husd',
        'brise',
        'lusd',
        'nft',
        'ewt',
        'nu',
        'chsb',
        'ln',
        'xcm',
        '10set',
        'alusd',
        'waxp',
        'hive',
        'vvs',
        'azero',
        'xch',
        'deso',
        'zen',
        'mdx',
        'sc',
        'scrt',
        'sxp',
        'rndr',
        'audio',
        'uma',
        'sgb',
        'inj',
        'dao',
        'sfm',
        'cet',
        'lsk',
        'pla',
        'skl',
        'dgb',
        'elon',
        'astr',
        'slp',
        'metis',
        'fx',
        'eurs',
        'pundix',
        'mxc',
        'kiro',
        'erg',
        'gfarm2',
        'stsol',
        'vgx',
        'ckb',
        'reef',
        'usdn',
        'ren',
        'lyxe',
        'api3',
        'coti',
        'looks',
        'dexe',
        'snt',
        'pltc',
        'uqc',
        'kub',
        'ceek',
        'usdx',
        'scho',
        'prom',
        'tribe',
        'veri',
        'pyr',
        'xsushi',
        'win',
        'med',
        'mpl',
        'pokt',
        'boba',
        'cvc',
        'xno',
        'sys',
        'gns',
        'steem',
        'orbs',
        'ardr',
        'mbox',
        'mvl',
        'tryb',
        'ever',
        'wrx',
        'kuji',
        'nmr',
        'pha',
        'mx',
        'ult',
        'seth2',
        'rad',
        'bsw',
        'aca',
        'people',
        'spell',
        'knc',
        'bico',
        'alice',
        'hxro',
        'eul',
        'multi',
        'plt',
        'c98',
        'raca',
        'exrd',
        'celr',
        'req',
        'plex',
        'powr',
        'bnt',
        'ctsi',
        'ilv',
        'ctc',
        'temple',
        'tel',
        'krd',
        'cfx',
        'uos',
        'dka',
        'nrv',
        'chr',
        'xprt',
        'dent',
        'fun',
        'asd',
        'arrr',
        'savax',
        'magic',
        'rlc',
        'bdx',
        'emaid',
        'xyo',
        'euroc',
        'quack',
        'strax',
        'wcfg',
        'cfg',
        'aethc',
        'tsuka',
        'btc.b',
        'qkc',
        'renbtc',
        'stpt',
        'canto',
        'vtho',
        'gal',
        'mlk',
        'fidu',
        'ton',
        'trac',
        'dock',
        'ctk',
        'ocean',
        'ray',
        'stg',
        'shr',
        'ogn',
        'mnw',
        'pcx',
        'sx',
        'susd',
        'joe',
        'nest',
        'kilt',
        'aurora',
        'h2o',
        'keep',
        'flexusd',
        'mtrg',
        'xvs',
        'bfc',
        'ant',
        'ccd',
        'ssv',
        'floki',
        'sfund',
        'cdt',
        'mtl',
        'ark',
        'sure',
        'xmon',
        'tlm',
        'btcst',
        'stmx',
        'itamcube',
        'koin',
        'lqty',
        'rev',
        'utk',
        'storj',
        'movr',
        'elf',
        'xido',
        'hez',
        'core',
        'efi',
        'aergo',
        'stars',
        'zz',
        'wozx',
        'fei',
        'ach',
        'fet',
        'sun',
        'agix',
        'gog',
        'sofi',
        'ride',
        'nkn',
        'mtd',
        'ads',
        'bor',
        'sweat',
        'oxt',
        'iq',
        'badger',
        'dodo',
        'fold',
        'xsgd',
        'rly',
        'xvg',
        'rep',
        'dawn',
        'gbex',
        'ygg',
        'strk',
        'beta',
        'mimatic',
        'alpha',
        'manc',
        'phb',
        'koda',
        'dpx',
        'dero',
        'ghst',
        'cusd',
        'meta',
        'ufo',
        'etn',
        'divi',
        'saitama',
        'mask',
        'dusk',
        'santos',
        'akt',
        'nxs',
        'bond',
        'rif',
        'mintme',
        'yfii',
        'wnxm',
        'bitci',
        'mft',
        'ibeur',
        'tlos',
        'dc',
        'ata',
        'alpaca',
        'fer',
        'xcad',
        'blid',
        'band',
        'wmt',
        'gods',
        'egc',
        'axn',
        'bake',
        'pols',
        'auction',
        'kishu',
        'ama',
        'tt',
        'usdk',
        'flm',
        'ageur',
        'zig',
        'poop',
        'dome',
        'vega',
        'sfp',
        'cbat',
        'vra',
        'ampl',
        'mln',
        'dola',
        'ousd',
        'kp3r',
        'cocos',
        'hydra',
        'pond',
        'tru',
        'dei',
        'ern',
        'idex',
        'nebl',
        'time',
        'loomold',
        'qrdo',
        'lcx',
        'avinoc',
        'perp',
        'hunt',
        'cweb',
        'b2m',
        'clv',
        'klv',
        'xki',
        'lto',
        'ava',
        'ron',
        'hc',
        'opt2',
        'astrafer',
        'kunci',
        'itheum',
        'leash',
        'tomo',
        'cqt',
        'volt',
        'orn',
        'regen',
        'super',
        'gxc',
        'wan',
        'forth',
        'sb',
    ],
    coingeckoTop500Names: [
        'Bitcoin',
        'Ethereum',
        'Tether',
        'BNB',
        'USD Coin',
        'XRP',
        'Binance USD',
        'Cardano',
        'Solana',
        'Dogecoin',
        'Polkadot',
        'Polygon',
        'Lido Staked Ether',
        'Shiba Inu',
        'Dai',
        'TRON',
        'Wrapped Bitcoin',
        'Avalanche',
        'Uniswap',
        'OKB',
        'LEO Token',
        'Litecoin',
        'Cosmos Hub',
        'Chainlink',
        'Ethereum Classic',
        'FTX',
        'Stellar',
        'Cronos',
        'Quant',
        'Monero',
        'NEAR Protocol',
        'Algorand',
        'Bitcoin Cash',
        'VeChain',
        'Terra Luna Classic',
        'Flow',
        'Filecoin',
        'Hedera',
        'ApeCoin',
        'Elrond',
        'WhiteBIT Token',
        'Frax',
        'Internet Computer',
        'Aptos',
        'Aave',
        'Tezos',
        'Chain',
        'Huobi',
        'Tokenize Xchange',
        'The Sandbox',
        'Decentraland',
        'Lido DAO',
        'EOS',
        'Theta Network',
        'Chiliz',
        'cUSDC',
        'KuCoin',
        'Bitcoin SV',
        'Pax Dollar',
        'Axie Infinity',
        'TrueUSD',
        'Maker',
        'Evmos',
        'BitTorrent',
        'USDD',
        'eCash',
        'Klaytn',
        'IOTA',
        'EthereumPoW',
        'Zcash',
        'cDAI',
        'PancakeSwap',
        'Gate',
        'The Graph',
        'NEO',
        'BTSE Token',
        'cETH',
        'Osmosis',
        'Helium',
        'Synthetix Network',
        'NEXO',
        'Fantom',
        'Arweave',
        'Radix',
        'Casper Network',
        'PAX Gold',
        'Curve DAO',
        'Kava',
        'BitDAO',
        'Ethereum Name Service',
        'Dash',
        'Trust Wallet',
        'Zilliqa',
        'THORChain',
        'Rocket Pool',
        'XDC Network',
        'Enjin Coin',
        'Basic Attention',
        'Tether Gold',
        'Stacks',
        'Frax Share',
        'Celsius Network',
        'Mina Protocol',
        'Decred',
        'DeFiChain',
        'Amp',
        'Terra',
        'Ravencoin',
        'cUSDT',
        'TerraClassicUSD',
        'Convex Finance',
        '1inch',
        'Maiar DEX',
        'Compound',
        'ECOMI',
        'Holo',
        'NEM',
        'Gemini Dollar',
        'Celo',
        'Waves',
        'Bitcoin Gold',
        'Immutable X',
        'Loopring',
        'Kusama',
        'Oasis Network',
        'Theta Fuel',
        'Nexus Mutual',
        'GMX',
        'Gnosis',
        'Olympus',
        'STEPN',
        'Qtum',
        'Sushi',
        'SafeMoon [OLD]',
        'OKC',
        'Serum',
        'IOST',
        'Golem',
        'JUST',
        'Gala',
        'Huobi BTC',
        'Kadena',
        'IoTeX',
        'Convex CRV',
        'Reserve Rights',
        'yearn.finance',
        'Polymath',
        'Ankr',
        'Escoin',
        'Livepeer',
        'Balancer',
        'Bone ShibaSwap',
        'OMG Network',
        'Marinade staked SOL',
        'Merit Circle',
        '0x',
        'Harmony',
        'Synapse',
        'Rocket Pool ETH',
        'Euro Tether',
        'dYdX',
        'Magic Internet Money',
        'Flux',
        'ICON',
        'Constellation',
        'Moonbeam',
        'WOO Network',
        'JUNO',
        'Optimism',
        'Ontology',
        'Ribbon Finance',
        'Bitgert',
        'HUSD',
        'Baby Doge Coin',
        'Energy Web',
        'APENFT',
        'SwissBorg',
        'Liquity USD',
        'NuCypher',
        'Coinmetro',
        'Tenset',
        'LINK',
        'Alchemix USD',
        'WAX',
        'Hive',
        'VVS Finance',
        'Decentralized Social',
        'Aleph Zero',
        'Chia',
        'Horizen',
        'Mdex',
        'Siacoin',
        'Secret',
        'SXP',
        'Render',
        'UMA',
        'Audius',
        'Injective',
        'DAO Maker',
        'Songbird',
        'SafeMoon',
        'CoinEx',
        'Lisk',
        'PlayDapp',
        'SKALE',
        'DigiByte',
        'Dogelon Mars',
        'Astar',
        'Smooth Love Potion',
        'Metis',
        'Function X',
        'Pundi X',
        'STASIS EURO',
        'MXC',
        'Kirobo',
        'Ergo',
        'Gains Farm',
        'Lido Staked SOL',
        'Voyager VGX',
        'Nervos Network',
        'Reef',
        'Neutrino USD',
        'REN',
        'LUKSO',
        'API3',
        'COTI',
        'LooksRare',
        'DeXe',
        'Status',
        'PlatonCoin',
        'Uquid Coin',
        'Bitkub Coin',
        'CEEK Smart VR',
        'USDX',
        'Scholarship Coin',
        'Tribe',
        'Prom',
        'Veritaseum',
        'xSUSHI',
        'Vulcan Forged',
        'WINkLink',
        'Medibloc',
        'Maple',
        'Pocket Network',
        'Civic',
        'Nano',
        'Boba Network',
        'Syscoin',
        'Gains Network',
        'Velas',
        'Steem',
        'Orbs',
        'Ardor',
        'Mobox',
        'MVL',
        'BiLira',
        'Everscale',
        'WazirX',
        'Kujira',
        'Numeraire',
        'Phala Network',
        'MX',
        'Shardus',
        'sETH2',
        'Radicle',
        'Biswap',
        'Acala',
        'ConstitutionDAO',
        'Spell',
        'Kyber Network Crystal',
        'Biconomy',
        'My Neighbor Alice',
        'Hxro',
        'Euler',
        'Multichain',
        'Poollotto.finance',
        'Coin98',
        'Radio Caca',
        'e-Radix',
        'Celer Network',
        'Request',
        'PLEX',
        'Power Ledger',
        'Bancor Network',
        'Cartesi',
        'Illuvium',
        'Creditcoin',
        'Telcoin',
        'TempleDAO',
        'Krypton DAO',
        'Conflux',
        'Ultra',
        'dKargo',
        'Nerve Finance',
        'Chromia',
        'Persistence',
        'Dent',
        'FUN',
        'AscendEx',
        'Pirate Chain',
        'BENQI Liquid Staked AVAX',
        'iExec RLC',
        'Magic',
        'Beldex',
        'MaidSafeCoin',
        'XYO Network',
        'Euro Coin',
        'Rich Quack',
        'Stratis',
        'Wrapped Centrifuge',
        'Centrifuge',
        'Ankr Reward-Bearing Staked ETH',
        'Bitcoin Avalanche Bridged (BTC.b)',
        'QuarkChain',
        'renBTC',
        'STP',
        'CANTO',
        'VeThor',
        'Galxe',
        'MiL.k Alliance',
        'Fidu',
        'Dejitaru Tsuka',
        'Tokamak Network',
        'OriginTrail',
        'Dock',
        'Shentu',
        'Ocean Protocol',
        'Raydium',
        'Stargate Finance',
        'Share',
        'Origin Protocol',
        'Morpheus Network',
        'ChainX',
        'SX Network',
        'sUSD',
        'JOE',
        'Nest Protocol',
        'KILT Protocol',
        'Aurora',
        'H2O Dao',
        'Keep Network',
        'flexUSD',
        'Meter Governance',
        'Venus',
        'Bifrost',
        'Aragon',
        'Concordium',
        'SSV Network',
        'FLOKI',
        'Seedify.fund',
        'Blox',
        'Metal',
        'Ark',
        'inSure DeFi',
        'XMON',
        'Alien Worlds',
        'BTC Standard Hashrate Token',
        'StormX',
        'CUBE',
        'Koinos',
        'Liquity',
        'Revain',
        'Utrust',
        'Storj',
        'Moonriver',
        'aelf',
        'Xido Finance',
        'Hermez Network',
        'cVault.finance',
        'Efinity',
        'Aergo',
        'Stargaze',
        'ZigZag',
        'Efforce',
        'Fei USD',
        'Alchemy Pay',
        'Fetch.ai',
        'Sun Token',
        'SingularityNET',
        'Guild of Guardians',
        'RAI Finance',
        'holoride',
        'NKN',
        'Minted',
        'Adshares',
        'BoringDAO [OLD]',
        'Sweatcoin - Sweat Economy',
        'Orchid Protocol',
        'IQ',
        'Badger DAO',
        'DODO',
        'Manifold Finance',
        'XSGD',
        'Rally',
        'Verge',
        'Augur',
        'Globiance Exchange',
        'Yield Guild Games',
        'Strike',
        'Dawn Protocol',
        'MAI',
        'Beta Finance',
        'Alpha Venture DAO',
        'Mancium',
        'Phoenix Global [OLD]',
        'Koda Cryptocurrency',
        'Dopex',
        'Dero',
        'Aavegotchi',
        'UFO Gaming',
        'Metadium',
        'Celo Dollar',
        'Electroneum',
        'Divi',
        'Saitama',
        'Mask Network',
        'DUSK Network',
        'Santos FC Fan Token',
        'Akash Network',
        'Nexus',
        'BarnBridge',
        'RSK Infrastructure Framework',
        'MintMe.com Coin',
        'DFI.money',
        'Wrapped NXM',
        'Hifi Finance',
        'Bitcicoin',
        'Iron Bank EURO',
        'Telos',
        'Dogechain',
        'XCAD Network',
        'Automata',
        'Alpaca Finance',
        'Ferro',
        'Bolide',
        'World Mobile Token',
        'Band Protocol',
        'Gods Unchained',
        'EverGrow Coin',
        'Axion',
        'BakerySwap',
        'Polkastarter',
        'Bounce',
        'Kishu Inu',
        'MrWeb Finance [OLD]',
        'ThunderCore',
        'USDK',
        'Flamingo Finance',
        'agEUR',
        'Zignaly',
        'Raresama',
        'Everdome',
        'Vega Protocol',
        'SafePal',
        'cBAT',
        'Verasity',
        'Ampleforth',
        'Enzyme',
        'Dola',
        'Keep3rV1',
        'Origin Dollar',
        'COCOS BCX',
        'Hydra',
        'Marlin',
        'TrueFi',
        'DEI',
        'Ethernity Chain',
        'IDEX',
        'Neblio',
        'chrono.tech',
        'Loom Network (OLD)',
        'LCX',
        'Qredo',
        'AVINOC',
        'Perpetual Protocol',
        'Hunt',
        'Coinweb',
        'Bit2Me',
        'Klever',
        'Clover Finance',
        'KI',
        'LTO Network',
        'Travala.com',
        'Ronin',
        'HyperCash',
        'Optimus OPT2',
        'Astrafer',
        'Kunci Coin',
        'Itheum',
        'Doge Killer',
        'TomoChain',
        'Covalent',
        'Volt Inu',
        'Orion Protocol',
        'Regen',
        'SuperFarm',
        'GXChain',
        'Vite',
        'Wanchain',
        'Ampleforth Governance',
    ],
    coingeckoTop500Ids: [
        'bitcoin',
        'ethereum',
        'tether',
        'binancecoin',
        'usd-coin',
        'ripple',
        'binance-usd',
        'cardano',
        'solana',
        'dogecoin',
        'polkadot',
        'matic-network',
        'staked-ether',
        'shiba-inu',
        'dai',
        'tron',
        'avalanche-2',
        'wrapped-bitcoin',
        'uniswap',
        'okb',
        'litecoin',
        'leo-token',
        'cosmos',
        'chainlink',
        'ethereum-classic',
        'ftx-token',
        'stellar',
        'crypto-com-chain',
        'quant-network',
        'monero',
        'near',
        'algorand',
        'bitcoin-cash',
        'vechain',
        'terra-luna',
        'flow',
        'filecoin',
        'apecoin',
        'hedera-hashgraph',
        'tokenize-xchange',
        'elrond-erd-2',
        'whitebit',
        'internet-computer',
        'tezos',
        'frax',
        'aptos',
        'chain-2',
        'huobi-token',
        'aave',
        'the-sandbox',
        'decentraland',
        'lido-dao',
        'eos',
        'theta-token',
        'chiliz',
        'compound-usd-coin',
        'kucoin-shares',
        'axie-infinity',
        'bitcoin-cash-sv',
        'paxos-standard',
        'true-usd',
        'maker',
        'evmos',
        'bittorrent',
        'usdd',
        'ecash',
        'iota',
        'ethereum-pow-iou',
        'zcash',
        'gatechain-token',
        'pancakeswap-token',
        'cdai',
        'klay-token',
        'the-graph',
        'neo',
        'compound-ether',
        'osmosis',
        'btse-token',
        'havven',
        'helium',
        'nexo',
        'fantom',
        'arweave',
        'pax-gold',
        'radix',
        'curve-dao-token',
        'casper-network',
        'kava',
        'bitdao',
        'trust-wallet-token',
        'dash',
        'ethereum-name-service',
        'zilliqa',
        'thorchain',
        'enjincoin',
        'basic-attention-token',
        'blockstack',
        'rocket-pool',
        'xdce-crowd-sale',
        'tether-gold',
        'frax-share',
        'mina-protocol',
        'celsius-degree-token',
        'decred',
        'defichain',
        'amp-token',
        'terra-luna-2',
        'ravencoin',
        '1inch',
        'convex-finance',
        'terrausd',
        'maiar-dex',
        'compound-usdt',
        'compound-governance-token',
        'nem',
        'holotoken',
        'ecomi',
        'gemini-dollar',
        'celo',
        'theta-fuel',
        'loopring',
        'waves',
        'kusama',
        'immutable-x',
        'nxm',
        'bitcoin-gold',
        'oasis-network',
        'gnosis',
        'gmx',
        'olympus',
        'qtum',
        'stepn',
        'sushi',
        'safemoon',
        'serum',
        'oec-token',
        'iostoken',
        'golem',
        'gala',
        'iotex',
        'kadena',
        'huobi-btc',
        'yearn-finance',
        'reserve-rights-token',
        'just',
        'convex-crv',
        'polymath',
        'balancer',
        'ankr',
        'livepeer',
        'escoin-token',
        'omisego',
        'bone-shibaswap',
        'msol',
        'rocket-pool-eth',
        '0x',
        'harmony',
        'synapse-2',
        'ribbon-finance',
        'tether-eurt',
        'merit-circle',
        'energy-web-token',
        'magic-internet-money',
        'icon',
        'dydx',
        'moonbeam',
        'zelcash',
        'woo-network',
        'constellation-labs',
        'optimism',
        'juno-network',
        'ontology',
        'baby-doge-coin',
        'nucypher',
        'husd',
        'tenset',
        'bitrise-token',
        'link',
        'liquity-usd',
        'coinmetro',
        'swissborg',
        'apenft',
        'wax',
        'alchemix-usd',
        'hive',
        'vvs-finance',
        'bitclout',
        'aleph-zero',
        'zencash',
        'chia',
        'siacoin',
        'mdex',
        'secret',
        'songbird',
        'swipe',
        'render-token',
        'audius',
        'injective-protocol',
        'uma',
        'dao-maker',
        'playdapp',
        'safemoon-2',
        'coinex-token',
        'lisk',
        'dogelon-mars',
        'digibyte',
        'skale',
        'astar',
        'smooth-love-potion',
        'gains-farm',
        'metis-token',
        'pundi-x-2',
        'lukso-token',
        'mxc',
        'fx-coin',
        'stasis-eurs',
        'lido-staked-sol',
        'kirobo',
        'ergo',
        'republic-protocol',
        'reef',
        'ethos',
        'nervos-network',
        'coti',
        'api3',
        'neutrino',
        'looksrare',
        'status',
        'dexe',
        'platoncoin',
        'bitkub-coin',
        'ceek',
        'gains-network',
        'tribe-2',
        'scholarship-coin',
        'usdx',
        'veritaseum',
        'prometeus',
        'medibloc',
        'xsushi',
        'syscoin',
        'vulcan-forged',
        'wink',
        'civic',
        'nano',
        'boba-network',
        'maple',
        'ardor',
        'pocket-network',
        'uquid-coin',
        'orbs',
        'mass-vehicle-ledger',
        'mobox',
        'everscale',
        'velas',
        'krypton-dao',
        'bilira',
        'wazirx',
        'steem',
        'numeraire',
        'kujira',
        'seth2',
        'constitutiondao',
        'hxro',
        'mx-token',
        'my-neighbor-alice',
        'kyber-network-crystal',
        'radicle',
        'spell-token',
        'shardus',
        'acala',
        'multichain',
        'biswap',
        'pha',
        'richquack',
        'biconomy',
        'coin98',
        'euler',
        'canto',
        'celer-network',
        'cartesi',
        'request-network',
        'illuvium',
        'milk-alliance',
        'radio-caca',
        'telcoin',
        'power-ledger',
        'bancor',
        'poollotto-finance',
        'ultra',
        'e-radix',
        'creditcoin-2',
        'conflux-token',
        'dkargo',
        'chromaway',
        'temple',
        'magic',
        'plex',
        'dent',
        'benqi-liquid-staked-avax',
        'pirate-chain',
        'nerve-finance',
        'funfair',
        'persistence',
        'beldex',
        'iexec-rlc',
        'maidsafecoin',
        'euro-coin',
        'ankreth',
        'wrapped-centrifuge',
        'centrifuge',
        'stratis',
        'stp-network',
        'xyo-network',
        'asd',
        'quark-chain',
        'aurora-near',
        'renbtc',
        'dejitaru-tsuka',
        'bitcoin-avalanche-bridged-btc-b',
        'project-galaxy',
        'origintrail',
        'fidu',
        'meter',
        'vethor-token',
        'tokamak-network',
        'raydium',
        'certik',
        'dock',
        'sx-network',
        'ocean-protocol',
        'origin-protocol',
        'sharering',
        'kilt-protocol',
        'morpheus-network',
        'joe',
        'chainx',
        'insure',
        'keep-network',
        'stargate-finance',
        'nusd',
        'aragon',
        'venus',
        'flex-usd',
        'bifrost',
        'h2o-dao',
        'ssv-network',
        'nest',
        'ark',
        'concordium',
        'metal',
        'floki',
        'koinos',
        'alien-worlds',
        'btc-standard-hashrate-token',
        'storm',
        'seedify-fund',
        'utrust',
        'liquity',
        'blox',
        'revain',
        'storj',
        'cube',
        'moonriver',
        'xmon',
        'aelf',
        'cvault-finance',
        'hermez-network-token',
        'efinity',
        'wozx',
        'guild-of-guardians',
        'aergo',
        'singularitynet',
        'alchemy-pay',
        'fei-usd',
        'fetch-ai',
        'zigzag-2',
        'minted',
        'everipedia',
        'sun-token',
        'stargaze',
        'sweatcoin',
        'rai-finance',
        'nkn',
        'adshares',
        'orchid-protocol',
        'augur',
        'boringdao-[old]',
        'holoride',
        'badger-dao',
        'dodo',
        'dogechain',
        'santos-fc-fan-token',
        'rally-2',
        'divi',
        'xido-finance',
        'dopex',
        'strike',
        'xsgd',
        'yield-guild-games',
        'dawn-protocol',
        'mancium',
        'verge',
        'beta-finance',
        'manifold-finance',
        'alpha-finance',
        'mimatic',
        'dero',
        'metadium',
        'celo-dollar',
        'red-pulse',
        'dusk-network',
        'koda-finance',
        'aavegotchi',
        'ufo-gaming',
        'mask-network',
        'saitama-inu',
        'akash-network',
        'electroneum',
        'globiance-exchange',
        'nexus',
        'rif-token',
        'barnbridge',
        'wrapped-nxm',
        'bitcicoin',
        'yfii-finance',
        'origin-dollar',
        'xcad-network',
        'automata',
        'iron-bank-euro',
        'mainframe',
        'webchain',
        'kishu-inu',
        'world-mobile-token',
        'telos',
        'alpaca-finance',
        'band-protocol',
        'ferro',
        'bolide',
        'thunder-token',
        'gods-unchained',
        'bakerytoken',
        'auction',
        'axion',
        'evergrowcoin',
        'vega-protocol',
        'polkastarter',
        'flamingo-finance',
        'mrweb-finance',
        'ampleforth',
        'everdome',
        'usdk',
        'ageur',
        'compound-basic-attention-token',
        'verasity',
        'zignaly',
        'safepal',
        'keep3rv1',
        'dola-usd',
        'truefi',
        'cocos-bcx',
        'raresama',
        'hydra',
        'marlin',
        'aurora-dao',
        'dei-token',
        'ethernity-chain',
        'loom-network',
        'chronobank',
        'qredo',
        'hunt-token',
        'step-app-fitfi',
        'lcx',
        'avinoc',
        'bit2me',
        'neblio',
        'melon',
        'optimus-opt2',
        'perpetual-protocol',
        'ronin',
        'klever',
        'coinweb',
        'volt-inu-2',
        'astrafer',
        'concierge-io',
        'leash',
        'tomochain',
        'clover-finance',
        'orion-protocol',
        'lto-network',
        'hshare',
        'regen',
        'itheum',
        'ki',
        'covalent',
        'unibright',
        'kunci-coin',
        'ampleforth-governance-token',
        'ellipsis',
        'superfarm',
    ],
    bannedTickers: ['ebtc', 'xbt'],
    bannedNames: ['ebitcoin'],
    fiatTickers: cashtabSettingsValidation.fiatCurrency,
    fiatNames,
    fiatSymbols,
};

const blacklists = Object.keys(tokenBlacklist);
let blacklist = [];
for (let i in blacklists) {
    blacklist = blacklist.concat(tokenBlacklist[blacklists[i]]);
}
// Remove spaces and case sensitivity
for (let i in blacklist) {
    blacklist[i] = blacklist[i]
        .toLowerCase()
        .trim()
        .split(' ')
        .filter(string => string)
        .join(' ');
}

export default blacklist;
