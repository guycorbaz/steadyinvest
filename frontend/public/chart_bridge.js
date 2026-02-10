window.setupDraggableHandles = function (chartId, salesStartValue, salesYears, epsStartValue, epsYears) {
    const chartDom = document.getElementById(chartId);
    if (!chartDom) return;

    let chart = echarts.getInstanceByDom(chartDom);
    if (!chart) {
        // Try again shortly if not initialized
        setTimeout(() => window.setupDraggableHandles(chartId, salesStartValue, salesYears, epsStartValue, epsYears), 100);
        return;
    }

    const updateHandles = () => {
        const option = chart.getOption();
        // We assume series 1 is Sales Projection and series 3 is EPS Projection
        // (based on the order in ssg_chart.rs)
        const salesSeries = option.series.find(s => s.name === 'Sales Projection');
        const epsSeries = option.series.find(s => s.name === 'EPS Projection');

        if (!salesSeries || !epsSeries) return;

        const lastIdx = salesSeries.data.length - 1;
        const salesPos = chart.convertToPixel({ gridIndex: 0 }, [lastIdx, salesSeries.data[lastIdx]]);
        const epsPos = chart.convertToPixel({ gridIndex: 0 }, [lastIdx, epsSeries.data[lastIdx]]);

        chart.setOption({
            graphic: [
                {
                    id: 'sales-handle',
                    type: 'circle',
                    position: salesPos,
                    shape: { r: 8 },
                    style: { fill: '#1DB954', stroke: '#fff', lineWidth: 2 },
                    draggable: true,
                    z: 100,
                    ondrag: function () {
                        const dataPos = chart.convertFromPixel({ gridIndex: 0 }, this.position);
                        const newValue = dataPos[1];
                        const cagr = (Math.pow(newValue / salesStartValue, 1 / salesYears) - 1) * 100;
                        console.log('[Handle] Sales (GREEN) dragged - new value:', newValue.toFixed(2), 'CAGR:', cagr.toFixed(2) + '%');
                        if (window.rust_update_sales_cagr) {
                            window.rust_update_sales_cagr(cagr);
                        }
                    }
                },
                {
                    id: 'eps-handle',
                    type: 'circle',
                    position: epsPos,
                    shape: { r: 8 },
                    style: { fill: '#3498DB', stroke: '#fff', lineWidth: 2 },
                    draggable: true,
                    z: 100,
                    ondrag: function () {
                        const dataPos = chart.convertFromPixel({ gridIndex: 0 }, this.position);
                        const newValue = dataPos[1];
                        const cagr = (Math.pow(newValue / epsStartValue, 1 / epsYears) - 1) * 100;
                        console.log('[Handle] EPS (BLUE) dragged - new value:', newValue.toFixed(2), 'CAGR:', cagr.toFixed(2) + '%');
                        if (window.rust_update_eps_cagr) {
                            window.rust_update_eps_cagr(cagr);
                        }
                    }
                }
            ]
        });
    };

    // Initial setup
    updateHandles();

    // Re-position handles on resize or data changes (if any)
    chart.on('finished', updateHandles);
    window.addEventListener('resize', () => chart.resize());
};
