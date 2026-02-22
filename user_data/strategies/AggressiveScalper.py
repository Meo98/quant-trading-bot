"""
Aggressive Scalping Strategy for Freqtrade
WARNING: This is a HIGH-RISK strategy designed for rapid trading with maximum aggression.
Use at your own risk. Can lead to significant losses.
"""

from freqtrade.strategy import IStrategy, IntParameter, DecimalParameter
from pandas import DataFrame
import talib.abstract as ta
import freqtrade.vendor.qtpylib.indicators as qtpylib


class AggressiveScalper(IStrategy):
    """
    Ultra-aggressive scalping strategy with:
    - Very frequent trades (scalping)
    - Tight stop-loss
    - Quick profit-taking
    - High risk tolerance
    """
    
    # Minimal ROI configuration - PROFITABEL nach Gebühren (0.3%)
    minimal_roi = {
        "0": 0.05,    # 5% profit target immediately (nach Gebühren: 4.7%)
        "10": 0.035,  # 3.5% after 10 minutes
        "20": 0.02,   # 2% after 20 minutes
        "40": 0.01    # 1% after 40 minutes (minimum Exit)
    }

    # Stoploss - etwas lockerer für weniger False-Exits
    stoploss = -0.03  # 3% stop loss (nach Gebühren: -3.3%)
    
    # Trailing stop (to lock in profits)
    trailing_stop = True
    trailing_stop_positive = 0.015  # Activate trailing at 1.5% profit
    trailing_stop_positive_offset = 0.025  # Offset at 2.5%
    trailing_only_offset_is_reached = True

    # Timeframe
    timeframe = '1m'  # 1-minute candles for maximum speed
    
    # Run "populate_indicators()" only for new candle
    process_only_new_candles = True
    
    # These values can be overridden in the config
    use_exit_signal = True
    exit_profit_only = True  # Nur profitable Trades mit Exit-Signal schließen
    ignore_roi_if_entry_signal = False

    # Number of candles the strategy requires before producing valid signals
    startup_candle_count: int = 30

    # Strategy parameters
    buy_rsi = IntParameter(15, 35, default=25, space="buy")  # Stärker oversold
    buy_rsi_fast = IntParameter(25, 45, default=35, space="buy")
    sell_rsi = IntParameter(65, 85, default=75, space="sell")  # Stärker overbought
    
    # Hyperoptable parameters
    buy_adx = DecimalParameter(20, 40, default=30, space="buy")
    buy_ema_short = IntParameter(3, 10, default=5, space="buy")
    buy_ema_long = IntParameter(15, 30, default=20, space="buy")

    def populate_indicators(self, dataframe: DataFrame, metadata: dict) -> DataFrame:
        """
        Add technical indicators for aggressive scalping
        """
        # RSI - Multiple timeframes for quick reactions
        dataframe['rsi'] = ta.RSI(dataframe, timeperiod=14)
        dataframe['rsi_fast'] = ta.RSI(dataframe, timeperiod=7)
        
        # EMA - Fast moving averages for quick signals
        dataframe['ema_short'] = ta.EMA(dataframe, timeperiod=self.buy_ema_short.value)
        dataframe['ema_long'] = ta.EMA(dataframe, timeperiod=self.buy_ema_long.value)
        
        # ADX - Trend strength
        dataframe['adx'] = ta.ADX(dataframe)
        
        # Bollinger Bands
        bollinger = qtpylib.bollinger_bands(qtpylib.typical_price(dataframe), window=20, stds=2)
        dataframe['bb_lowerband'] = bollinger['lower']
        dataframe['bb_middleband'] = bollinger['mid']
        dataframe['bb_upperband'] = bollinger['upper']
        
        # Volume
        dataframe['volume_mean'] = dataframe['volume'].rolling(window=20).mean()
        
        # MACD - Fast settings for scalping
        macd = ta.MACD(dataframe, fastperiod=6, slowperiod=13, signalperiod=5)
        dataframe['macd'] = macd['macd']
        dataframe['macdsignal'] = macd['macdsignal']
        dataframe['macdhist'] = macd['macdhist']

        return dataframe

    def populate_entry_trend(self, dataframe: DataFrame, metadata: dict) -> DataFrame:
        """
        Entry signals - SELEKTIVER für profitablere Trades
        Strengere Bedingungen = weniger Trades, höhere Gewinnrate
        """
        dataframe.loc[
            (
                # Condition 1: Starker Oversold-Bounce mit Bestätigung
                (
                    (dataframe['rsi'] < self.buy_rsi.value) &  # Stärker oversold
                    (dataframe['rsi_fast'] < self.buy_rsi_fast.value) &
                    (dataframe['ema_short'] > dataframe['ema_long']) &
                    (dataframe['macd'] > dataframe['macdsignal']) &  # MACD Bestätigung
                    (dataframe['volume'] > dataframe['volume_mean'] * 1.5)  # Höheres Volumen
                ) |
                # Condition 2: Bollinger Band Bounce mit starkem Momentum
                (
                    (dataframe['close'] < dataframe['bb_lowerband']) &  # Unter BB
                    (dataframe['macd'] > dataframe['macdsignal']) &
                    (dataframe['adx'] > self.buy_adx.value) &  # Trend-Bestätigung
                    (dataframe['volume'] > dataframe['volume_mean'] * 2.0)  # Sehr hohes Volumen
                )
            ),
            'enter_long'] = 1

        return dataframe

    def populate_exit_trend(self, dataframe: DataFrame, metadata: dict) -> DataFrame:
        """
        Exit signals - FAST exits
        Take profits quickly or cut losses
        """
        dataframe.loc[
            (
                # Condition 1: RSI overbought
                (
                    (dataframe['rsi_fast'] > self.sell_rsi.value)
                ) |
                # Condition 2: Price near upper Bollinger Band
                (
                    (dataframe['close'] > dataframe['bb_upperband'] * 0.99)
                ) |
                # Condition 3: EMA crossover down
                (
                    (dataframe['ema_short'] < dataframe['ema_long']) &
                    (dataframe['ema_short'].shift(1) >= dataframe['ema_long'].shift(1))
                ) |
                # Condition 4: MACD turning negative
                (
                    (dataframe['macd'] < dataframe['macdsignal']) &
                    (dataframe['macdhist'] < 0)
                )
            ),
            'exit_long'] = 1

        return dataframe
