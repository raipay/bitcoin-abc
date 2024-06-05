#!/usr/bin/env python3

# Copyright (c) 2024 The Bitcoin developers
# Distributed under the MIT software license, see the accompanying
# file COPYING or http://www.opensource.org/licenses/mit-license.php.

from subprocess import run
import os


class NodeBuild:
    def __init__(self, *, name: str, git_url: str) -> None:
        self.name = name
        self.git_url = git_url
        self.repo_dir = None
        self.build_dir = None

    def download_src(self, nodes_dir: str) -> None:
        self.repo_dir = os.path.join(nodes_dir, self.name)
        self.build_dir = os.path.join(self.repo_dir, 'build')
        print('Updating', self.repo_dir)
        if os.path.exists(self.repo_dir):
            run(['git', 'pull'], cwd=self.repo_dir)
        else:
            run(['git', 'clone', self.git_url, self.repo_dir])

    def patch(self, base_dir: str):
        patch_path = os.path.join(base_dir, f'{self.name}.patch')
        run(['git', 'apply', patch_path], cwd=self.repo_dir)

    def build(self):
        os.makedirs(self.build_dir, exist_ok=True)
        run(
            [
                'cmake',
                self.repo_dir,
                '-GNinja',
                '-DBUILD_BITCOIN_ZMQ=off',
                '-DBUILD_BITCOIN_CHRONIK=on',
            ],
            cwd=self.build_dir,
        )
        run(['ninja'], cwd=self.build_dir)

    def test(self):
        run(['ninja', 'check-functional'], cwd=self.build_dir)


def main():
    # git hash: 7ef8bf2cd9b980dd75ea0b2cbea3ca5d161970ed
    bchn = NodeBuild(name='bchn', git_url='https://gitlab.com/bitcoin-cash-node/bitcoin-cash-node.git')

    base_dir = os.path.dirname(__file__)
    nodes_dir = os.path.join(base_dir, 'nodes')
    os.makedirs(nodes_dir, exist_ok=True)

    bchn.download_src(nodes_dir)
    bchn.patch(base_dir)
    bchn.build()
    # bchn.test()


if __name__ == '__main__':
    main()
