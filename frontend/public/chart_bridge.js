window.setupDraggableHandles = function (chartId, salesStartValue, salesYears, epsStartValue, epsYears, ptpStartValue, ptpYears) {
    const chartDom = document.getElementById(chartId);
    if (!chartDom) return;

    let chart = echarts.getInstanceByDom(chartDom);
    if (!chart) {
        // Try again shortly if not initialized
        setTimeout(() => window.setupDraggableHandles(chartId, salesStartValue, salesYears, epsStartValue, epsYears, ptpStartValue, ptpYears), 100);
        return;
    }

    const updateHandles = () => {
        const option = chart.getOption();
        const salesSeries = option.series.find(s => s.name && s.name.includes('Sales Est.'));
        const epsSeries = option.series.find(s => s.name && s.name.includes('EPS Est.'));
        const ptpSeries = option.series.find(s => s.name && s.name.includes('PTP Est.'));

        if (!salesSeries || !epsSeries) return;

        const lastIdx = salesSeries.data.length - 1;
        const salesPos = chart.convertToPixel({ gridIndex: 0 }, [lastIdx, salesSeries.data[lastIdx]]);
        const epsPos = chart.convertToPixel({ gridIndex: 0 }, [lastIdx, epsSeries.data[lastIdx]]);

        const graphics = [
            {
                id: 'sales-handle',
                type: 'circle',
                position: salesPos,
                shape: { r: 8 },
                style: {
                    fill: '#1DB954',
                    stroke: '#E0E0E0',
                    lineWidth: 2
                },
                emphasis: {
                    style: {
                        shadowBlur: 8,
                        shadowColor: 'rgba(29, 185, 84, 0.5)'
                    }
                },
                draggable: true,
                z: 100,
                cursor: 'grab',
                ondrag: function () {
                    this.cursor = 'grabbing';
                    const dataPos = chart.convertFromPixel({ gridIndex: 0 }, this.position);
                    const newValue = dataPos[1];
                    const cagr = (Math.pow(newValue / salesStartValue, 1 / salesYears) - 1) * 100;
                    if (window.rust_update_sales_cagr) {
                        window.rust_update_sales_cagr(cagr);
                    }
                },
                ondragend: function () {
                    this.cursor = 'grab';
                }
            },
            {
                id: 'eps-handle',
                type: 'circle',
                position: epsPos,
                shape: { r: 8 },
                style: {
                    fill: '#3498DB',
                    stroke: '#E0E0E0',
                    lineWidth: 2
                },
                emphasis: {
                    style: {
                        shadowBlur: 8,
                        shadowColor: 'rgba(52, 152, 219, 0.5)'
                    }
                },
                draggable: true,
                z: 100,
                cursor: 'grab',
                ondrag: function () {
                    this.cursor = 'grabbing';
                    const dataPos = chart.convertFromPixel({ gridIndex: 0 }, this.position);
                    const newValue = dataPos[1];
                    const cagr = (Math.pow(newValue / epsStartValue, 1 / epsYears) - 1) * 100;
                    if (window.rust_update_eps_cagr) {
                        window.rust_update_eps_cagr(cagr);
                    }
                },
                ondragend: function () {
                    this.cursor = 'grab';
                }
            }
        ];

        // Add PTP handle if PTP projection series exists
        if (ptpSeries && ptpStartValue > 0) {
            const ptpLastIdx = ptpSeries.data.length - 1;
            const ptpPos = chart.convertToPixel({ gridIndex: 0 }, [ptpLastIdx, ptpSeries.data[ptpLastIdx]]);
            graphics.push({
                id: 'ptp-handle',
                type: 'circle',
                position: ptpPos,
                shape: { r: 8 },
                style: {
                    fill: '#E74C3C',
                    stroke: '#E0E0E0',
                    lineWidth: 2
                },
                emphasis: {
                    style: {
                        shadowBlur: 8,
                        shadowColor: 'rgba(231, 76, 60, 0.5)'
                    }
                },
                draggable: true,
                z: 100,
                cursor: 'grab',
                ondrag: function () {
                    this.cursor = 'grabbing';
                    const dataPos = chart.convertFromPixel({ gridIndex: 0 }, this.position);
                    const newValue = dataPos[1];
                    const cagr = (Math.pow(newValue / ptpStartValue, 1 / ptpYears) - 1) * 100;
                    if (window.rust_update_ptp_cagr) {
                        window.rust_update_ptp_cagr(cagr);
                    }
                },
                ondragend: function () {
                    this.cursor = 'grab';
                }
            });
        }

        chart.setOption({ graphic: graphics });
    };

    // Initial setup
    updateHandles();

    // Remove any prior 'finished' listener to prevent stacking on repeated calls
    if (chart.__ssgHandleListener) {
        chart.off('finished', chart.__ssgHandleListener);
    }
    chart.__ssgHandleListener = updateHandles;
    chart.on('finished', updateHandles);

    // Remove prior resize listener to prevent stacking on repeated calls
    if (chart.__ssgResizeListener) {
        window.removeEventListener('resize', chart.__ssgResizeListener);
    }
    chart.__ssgResizeListener = () => chart.resize();
    window.addEventListener('resize', chart.__ssgResizeListener);
};

/**
 * Adds NAIC-style vertical price bars (low→high) to the SSG chart.
 * Called from Rust after WasmRenderer.render() because charming's RawString
 * doesn't work through serde_wasm_bindgen (Custom series renderItem needs
 * a real JS function, not a string).
 *
 * @param {string} chartId - DOM element id of the chart container
 * @param {string} priceDataJson - JSON array of [low, high] pairs per year
 */
window.addPriceBars = function (chartId, priceDataJson) {
    const chartDom = document.getElementById(chartId);
    if (!chartDom) return;
    const chart = echarts.getInstanceByDom(chartDom);
    if (!chart) return;

    const priceData = JSON.parse(priceDataJson);
    const data = priceData.map((d, i) => [i, d[0], d[1]]);

    // Append the custom series via setOption merge mode.
    // Use a sparse array so existing series (indices 0..N-1) are untouched.
    const opt = chart.getOption();
    const existingCount = (opt.series || []).length;
    const sparse = new Array(existingCount + 1);
    sparse[existingCount] = {
        type: 'custom',
        name: 'Stock Price',
        renderItem: function (params, api) {
            var low = api.coord([api.value(0), api.value(1)]);
            var high = api.coord([api.value(0), api.value(2)]);
            return {
                type: 'line',
                shape: { x1: low[0], y1: low[1], x2: high[0], y2: high[1] },
                style: { stroke: '#B0B0B0', lineWidth: 2 }
            };
        },
        data: data
    };
    chart.setOption({ series: sparse });
};

window.captureChartAsDataURL = function (chartId) {
    const chartDom = document.getElementById(chartId);
    if (!chartDom) {
        console.warn('[captureChartAsDataURL] DOM element not found:', chartId);
        return null;
    }
    let chart = echarts.getInstanceByDom(chartDom);
    if (!chart) {
        console.warn('[captureChartAsDataURL] No ECharts instance for:', chartId);
        return null;
    }
    try {
        const bg = (chart.getOption().backgroundColor) || '#1a1a2e';
        return chart.getDataURL({ type: 'png', pixelRatio: 2, backgroundColor: bg });
    } catch (e) {
        console.warn('[captureChartAsDataURL] Export failed:', e);
        return null;
    }
};
