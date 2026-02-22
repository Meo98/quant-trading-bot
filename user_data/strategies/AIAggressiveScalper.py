"""
AI-Enhanced Aggressive Scalping Strategy
Uses LLM (Gemini/Claude) for intelligent exit decisions
"""

from freqtrade.strategy import IStrategy, IntParameter, DecimalParameter
from pandas import DataFrame
import talib.abstract as ta
import freqtrade.vendor.qtpylib.indicators as qtpylib
from datetime import datetime
import os
import sys

# Add user_data to path for imports
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..'))

try:
    from llm_advisor import LLMTradeAdvisor
    LLM_AVAILABLE = True
except Exception as e:
    print(f"LLM Advisor not available: {e}")
    LLM_AVAILABLE = False


class AIAggressiveScalper(IStrategy):
    """
    LLM-enhanced scalping strategy with intelligent exits
    """
    
    # Minimal ROI configuration - PROFITABEL nach Gebühren (0.3%)
    minimal_roi = {
        "0": 0.08,    # 8% profit target (LLM kann früher exitieren)
        "15": 0.05,   # 5% after 15 minutes
        "30": 0.03,   # 3% after 30 minutes
        "60": 0.015   # 1.5% after 60 minutes
    }

    # Stoploss
    stoploss = -0.10  # 10% stop loss (Riskant!)
    
    # Trailing stop (Backup falls LLM ausfällt)
    trailing_stop = True
    trailing_stop_positive = 0.02  # Activate at 2%
    trailing_stop_positive_offset = 0.03  # Trail at 3%
    trailing_only_offset_is_reached = True

    # Timeframe
    timeframe = '1m'
    
    process_only_new_candles = False
    
    use_exit_signal = True
    exit_profit_only = True
    ignore_roi_if_entry_signal = False

    startup_candle_count: int = 30

    # Strategy parameters
    buy_rsi = IntParameter(15, 35, default=35, space="buy")
    buy_rsi_fast = IntParameter(25, 45, default=35, space="buy")
    sell_rsi = IntParameter(65, 85, default=75, space="sell")
    
    buy_adx = DecimalParameter(20, 40, default=20, space="buy")
    buy_ema_short = IntParameter(3, 10, default=5, space="buy")
    buy_ema_long = IntParameter(15, 30, default=20, space="buy")
    
    # LLM parameters
    use_llm_for_exit = True  # Enable LLM exits
    use_llm_for_entry = False  # Disable for now (too slow)
    llm_provider = "gemini"  # or "claude"
    llm_exit_confidence_threshold = 0.6  # Min confidence to follow LLM

    def __init__(self, config: dict) -> None:
        super().__init__(config)
        
        # Initialize LLM advisor
        if LLM_AVAILABLE and self.use_llm_for_exit:
            try:
                self.llm_advisor = LLMTradeAdvisor(provider=self.llm_provider)
                print(f"✅ LLM Advisor initialized ({self.llm_provider})")
            except Exception as e:
                print(f"❌ LLM init failed: {e}")
                self.llm_advisor = None
        else:
            self.llm_advisor = None

    def populate_indicators(self, dataframe: DataFrame, metadata: dict) -> DataFrame:
        """Add technical indicators"""
        
        # RSI
        dataframe['rsi'] = ta.RSI(dataframe, timeperiod=14)
        dataframe['rsi_fast'] = ta.RSI(dataframe, timeperiod=7)
        
        # EMA
        dataframe['ema_short'] = ta.EMA(dataframe, timeperiod=self.buy_ema_short.value)
        dataframe['ema_long'] = ta.EMA(dataframe, timeperiod=self.buy_ema_long.value)
        
        # ADX
        dataframe['adx'] = ta.ADX(dataframe)
        
        # Bollinger Bands
        bollinger = qtpylib.bollinger_bands(qtpylib.typical_price(dataframe), window=20, stds=2)
        dataframe['bb_lowerband'] = bollinger['lower']
        dataframe['bb_middleband'] = bollinger['mid']
        dataframe['bb_upperband'] = bollinger['upper']
        
        # Distance from BB
        dataframe['bb_lower_dist'] = ((dataframe['close'] - dataframe['bb_lowerband']) / dataframe['close']) * 100
        
        # Volume
        dataframe['volume_mean'] = dataframe['volume'].rolling(window=20).mean()
        dataframe['volume_spike'] = ((dataframe['volume'] / dataframe['volume_mean']) - 1) * 100
        
        # MACD
        macd = ta.MACD(dataframe, fastperiod=6, slowperiod=13, signalperiod=5)
        dataframe['macd'] = macd['macd']
        dataframe['macdsignal'] = macd['macdsignal']
        dataframe['macdhist'] = macd['macdhist']

        return dataframe

    def populate_entry_trend(self, dataframe: DataFrame, metadata: dict) -> DataFrame:
        """Entry signals - SELEKTIVER für profitablere Trades"""
        
        dataframe.loc[
            (
                # Condition 1: Starker Oversold-Bounce mit Bestätigung
                (
                    (dataframe['rsi'] < self.buy_rsi.value) &
                    (dataframe['rsi_fast'] < self.buy_rsi_fast.value) &
                    (dataframe['ema_short'] > dataframe['ema_long']) &
                    (dataframe['macd'] > dataframe['macdsignal']) &
                    (dataframe['volume'] > dataframe['volume_mean'] * 1.5)
                ) |
                # Condition 2: Bollinger Band Bounce mit starkem Momentum
                (
                    (dataframe['close'] < dataframe['bb_lowerband']) &
                    (dataframe['macd'] > dataframe['macdsignal']) &
                    (dataframe['adx'] > self.buy_adx.value) &
                    (dataframe['volume'] > dataframe['volume_mean'] * 2.0)
                )
            ),
            'enter_long'] = 1

        return dataframe

    def populate_exit_trend(self, dataframe: DataFrame, metadata: dict) -> DataFrame:
        """
        Exit signals - Enhanced with LLM intelligence
        """
        
        # Standard technical exits (fallback)
        dataframe.loc[
            (
                (dataframe['rsi_fast'] > self.sell_rsi.value) |
                (dataframe['close'] > dataframe['bb_upperband'] * 0.99) |
                (
                    (dataframe['ema_short'] < dataframe['ema_long']) &
                    (dataframe['ema_short'].shift(1) >= dataframe['ema_long'].shift(1))
                ) |
                (
                    (dataframe['macd'] < dataframe['macdsignal']) &
                    (dataframe['macdhist'] < 0)
                )
            ),
            'exit_long'] = 1

        return dataframe
    
    def custom_exit(self, pair: str, trade, current_time, current_rate,
                    current_profit, **kwargs) -> tuple:
        """
        Custom exit with LLM intelligence
        Returns: (exit_flag, exit_type)
        """
        
        # Skip if LLM not available
        if not self.llm_advisor or not self.use_llm_for_exit:
            return None, None
        
        # Only check LLM if we're in profit (> 1.5%)
        if current_profit < 0.015:
            return None, None
        
        # Get current dataframe
        dataframe, _ = self.dp.get_analyzed_dataframe(pair, self.timeframe)
        if dataframe.empty:
            return None, None
        
        last_candle = dataframe.iloc[-1]
        
        # Calculate time in trade
        time_in_trade = (current_time - trade.open_date_utc).total_seconds() / 60
        
        # Get LLM decision
        try:
            llm_decision = self.llm_advisor.should_exit_trade(
                pair=pair,
                entry_price=trade.open_rate,
                current_price=current_rate,
                current_profit_pct=current_profit * 100,
                time_in_trade_minutes=int(time_in_trade),
                volume_24h_change_pct=last_candle['volume_spike'],
                rsi=last_candle['rsi'],
                macd_histogram=last_candle['macdhist']
            )
            
            # Log LLM decision
            print(f"🤖 LLM: {pair} | Profit: {current_profit*100:.2f}% | "
                  f"Action: {llm_decision['action']} | "
                  f"Confidence: {llm_decision['confidence']:.2f} | "
                  f"Reason: {llm_decision['reasoning']}")
            
            # Exit if LLM says EXIT with high confidence
            if (llm_decision['action'] == 'EXIT' and 
                llm_decision['confidence'] >= self.llm_exit_confidence_threshold):
                return 'llm_intelligent_exit', llm_decision['reasoning']
        
        except Exception as e:
            print(f"❌ LLM Error: {e}")
        
        return None, None
