// Copyright (c) 2024 The Bitcoin developers
// Distributed under the MIT software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

// @ts-check
// `@type` JSDoc annotations allow editor autocompletion and type checking
// (when paired with `@ts-check`).
// There are various equivalent ways to declare your Docusaurus config.
// See: https://docusaurus.io/docs/api/docusaurus-config

import {themes as prismThemes} from 'prism-react-renderer';

/** @type {import('@docusaurus/types').Config} */
const config = {
  title: 'Chronik Documentation',
  tagline: 'Chronik is a fast and reliable indexer built into the Bitcoin ABC node software',
  favicon: 'img/favicon.ico',

  // Set the production url of your site here
  url: 'https://docs.chronik.xyz/',
  // Set the /<baseUrl>/ pathname under which your site is served
  // For GitHub pages deployment, it is often '/<projectName>/'
  baseUrl: '/',

  organizationName: 'Bitcoin ABC',
  projectName: 'Bitcoin ABC',

  onBrokenLinks: 'throw',
  onBrokenMarkdownLinks: 'warn',

  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  plugins: [
    'docusaurus-plugin-sass',
    [
      'docusaurus-plugin-typedoc',
      {
        entryPoints: ['../../modules/chronik-client/'],
        entryPointStrategy: 'packages',
        out: './1-chronik-js/api',
        sidebar: {
          categoryLabel: 'API Reference',
          position: 10,
        },
      },
    ],
  ],
  themes: ['@docusaurus/theme-live-codeblock'],

  presets: [
    [
      'classic',
      /** @type {import('@docusaurus/preset-classic').Options} */
      ({
        docs: {
          sidebarPath: './sidebars.js',
          editUrl:
            'https://github.com/Bitcoin-ABC/bitcoin-abc/tree/master/doc/chronik/',
        },
        theme: {
          customCss: './src/css/custom.css',
        },
      }),
    ],
  ],

  themeConfig:
    /** @type {import('@docusaurus/preset-classic').ThemeConfig} */
    ({
      image: 'img/chronik-social-card.jpg',
      navbar: {
        title: 'Chronik Documentation',
        logo: {
          alt: 'Chronik Logo',
          src: 'img/logo.svg',
        },
        items: [
          {
            type: 'docSidebar',
            sidebarId: 'tutorialSidebar',
            position: 'left',
            label: 'Chronik Documentation',
          },
          {
            href: 'https://github.com/Bitcoin-ABC/bitcoin-abc',
            label: 'GitHub',
            position: 'right',
          },
        ],
      },
      footer: {
        style: 'dark',
        links: [
          {
            title: 'Docs',
            items: [
              {
                label: 'Chronik Documentation',
                to: '/docs/intro',
              },
            ],
          },
          {
            title: 'Community',
            items: [
              {
                label: 'Twitter',
                href: 'https://twitter.com/eCashOfficial',
              },
            ],
          },
          {
            title: 'More',
            items: [
              {
                label: 'GitHub',
                href: 'https://github.com/Bitcoin-ABC/bitcoin-abc',
              },
            ],
          },
        ],
        copyright: `Copyright © ${new Date().getFullYear()} the Bitcoin Developers. Built with Docusaurus.`,
      },
      prism: {
        theme: prismThemes.github,
        darkTheme: prismThemes.dracula,
        additionalLanguages: ['nginx', 'bash', 'toml', 'json'],
      },
    }),
};

export default config;
