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
    window.addEventListener('resize', () => chart.resize());
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
