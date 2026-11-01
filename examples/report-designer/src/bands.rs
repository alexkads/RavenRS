//! Addressing the bands of a report in design order.

use ravenrs_report::{Band, Report};

/// Which band of the report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BandRef {
    ReportHeader,
    PageHeader,
    GroupHeader(usize),
    Detail,
    DetailReportHeader(usize),
    DetailReportDetail(usize),
    DetailReportFooter(usize),
    GroupFooter(usize),
    ReportFooter,
    PageFooter,
}

impl BandRef {
    /// The bands present in `report`, top to bottom as the designer shows them.
    pub fn all(report: &Report) -> Vec<BandRef> {
        let mut out = Vec::new();
        if report.report_header.is_some() {
            out.push(Self::ReportHeader);
        }
        if report.page_header.is_some() {
            out.push(Self::PageHeader);
        }
        for (i, g) in report.groups.iter().enumerate() {
            if g.header.is_some() {
                out.push(Self::GroupHeader(i));
            }
        }
        out.push(Self::Detail);
        for (i, d) in report.detail_reports.iter().enumerate() {
            if d.header.is_some() {
                out.push(Self::DetailReportHeader(i));
            }
            out.push(Self::DetailReportDetail(i));
            if d.footer.is_some() {
                out.push(Self::DetailReportFooter(i));
            }
        }
        for (i, g) in report.groups.iter().enumerate().rev() {
            if g.footer.is_some() {
                out.push(Self::GroupFooter(i));
            }
        }
        if report.report_footer.is_some() {
            out.push(Self::ReportFooter);
        }
        if report.page_footer.is_some() {
            out.push(Self::PageFooter);
        }
        out
    }

    /// The band in `report`.
    pub fn get(self, report: &Report) -> Option<&Band> {
        match self {
            Self::ReportHeader => report.report_header.as_ref(),
            Self::PageHeader => report.page_header.as_ref(),
            Self::GroupHeader(i) => report.groups.get(i)?.header.as_ref(),
            Self::GroupFooter(i) => report.groups.get(i)?.footer.as_ref(),
            Self::Detail => Some(&report.detail),
            Self::DetailReportHeader(i) => report.detail_reports.get(i)?.header.as_ref(),
            Self::DetailReportDetail(i) => Some(&report.detail_reports.get(i)?.detail),
            Self::DetailReportFooter(i) => report.detail_reports.get(i)?.footer.as_ref(),
            Self::ReportFooter => report.report_footer.as_ref(),
            Self::PageFooter => report.page_footer.as_ref(),
        }
    }

    /// The band in `report`, for editing.
    pub fn get_mut(self, report: &mut Report) -> Option<&mut Band> {
        match self {
            Self::ReportHeader => report.report_header.as_mut(),
            Self::PageHeader => report.page_header.as_mut(),
            Self::GroupHeader(i) => report.groups.get_mut(i)?.header.as_mut(),
            Self::GroupFooter(i) => report.groups.get_mut(i)?.footer.as_mut(),
            Self::Detail => Some(&mut report.detail),
            Self::DetailReportHeader(i) => report.detail_reports.get_mut(i)?.header.as_mut(),
            Self::DetailReportDetail(i) => Some(&mut report.detail_reports.get_mut(i)?.detail),
            Self::DetailReportFooter(i) => report.detail_reports.get_mut(i)?.footer.as_mut(),
            Self::ReportFooter => report.report_footer.as_mut(),
            Self::PageFooter => report.page_footer.as_mut(),
        }
    }

    /// The band's name, as the designer shows it.
    pub fn title(self, report: &Report) -> String {
        match self {
            Self::ReportHeader => "Cabeçalho do relatório".into(),
            Self::PageHeader => "Cabeçalho da página".into(),
            Self::GroupHeader(i) => format!("Cabeçalho do grupo · {}", group_expr(report, i)),
            Self::GroupFooter(i) => format!("Rodapé do grupo · {}", group_expr(report, i)),
            Self::Detail => "Detalhe".into(),
            Self::DetailReportHeader(i) => format!("Cabeçalho · {}", detail_member(report, i)),
            Self::DetailReportDetail(i) => format!("Detalhe · {}", detail_member(report, i)),
            Self::DetailReportFooter(i) => format!("Rodapé · {}", detail_member(report, i)),
            Self::ReportFooter => "Rodapé do relatório".into(),
            Self::PageFooter => "Rodapé da página".into(),
        }
    }

    /// A stable number for widget keys.
    pub fn key(self) -> u64 {
        match self {
            Self::ReportHeader => 1,
            Self::PageHeader => 2,
            Self::Detail => 3,
            Self::ReportFooter => 4,
            Self::PageFooter => 5,
            Self::GroupHeader(i) => 100 + i as u64,
            Self::GroupFooter(i) => 200 + i as u64,
            Self::DetailReportHeader(i) => 300 + i as u64,
            Self::DetailReportDetail(i) => 400 + i as u64,
            Self::DetailReportFooter(i) => 500 + i as u64,
        }
    }
}

fn group_expr(report: &Report, i: usize) -> String {
    report.groups.get(i).map(|g| g.expression.clone()).unwrap_or_default()
}

fn detail_member(report: &Report, i: usize) -> String {
    report.detail_reports.get(i).map(|d| d.data_member.clone()).unwrap_or_default()
}
