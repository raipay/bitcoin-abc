const path = require('path');

module.exports = {
    entry: './extension/src/components/App.js', // Main file of your React app
    output: {
        path: path.resolve(__dirname, 'extension/public'),
        filename: 'bundle.js', // Output bundle file
    },
    module: {
        rules: [
            {
                test: /\.(js|jsx)$/,
                exclude: /node_modules/,
                use: {
                    loader: 'babel-loader',
                },
            },
            {
                test: /\.css$/,
                use: ['style-loader', 'css-loader']
            },
            {
                test: /\.less$/,
                use: [
                    'style-loader',
                    'css-loader',
                    {
                        loader: 'less-loader',
                        options: {
                            lessOptions: {
                                javascriptEnabled: true,
                            },
                        },
                    },
                ],
            },
            {
                test: /\.(png|svg)$/,
                use: ['file-loader'],
            },
        ],
    },
    resolve: {
        extensions: ['.js', '.jsx'], // Automatically resolve these file types
        fallback: {
            crypto: require.resolve('crypto-browserify'),
            stream: require.resolve('stream-browserify'),
        },
        alias: {
            // Align with jsconfig.json
            src: path.resolve(__dirname, 'src')
        }
    },
    // Add plugins if needed (e.g., HtmlWebpackPlugin, MiniCssExtractPlugin)
};

