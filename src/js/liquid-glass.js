/**
 * 液态玻璃效果 - CSS 方案
 * 使用 backdrop-filter 实现模糊，CSS 渐变实现高光/反射
 * 不使用 WebGL（避免黑色画布覆盖问题）
 */

(function() {
    'use strict';

    // 公开 API（兼容 app.js 调用）
    window.LiquidGlass = {
        init: function() {
            // CSS 已处理玻璃效果，无需额外初始化
        },

        setRefraction: function(value) {
            document.documentElement.style.setProperty('--refraction-strength', value);
        },

        setChromatic: function(value) {
            document.documentElement.style.setProperty('--chromatic-strength', value);
        },

        setBlur: function(value) {
            document.documentElement.style.setProperty('--blur-intensity', value + 'px');
        },

        refresh: function() {
            // CSS 自动处理，无需刷新
        }
    };

    // 兼容 app.js 中的 updateGlassRefraction / updateGlassChromatic
    window.updateGlassRefraction = function(value) {
        window.LiquidGlass.setRefraction(value);
    };
    window.updateGlassChromatic = function(value) {
        window.LiquidGlass.setChromatic(value);
    };
})();
