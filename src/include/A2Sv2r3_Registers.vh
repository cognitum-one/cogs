//================================================================================================================================
//
//   Copyright (c) 2023 Advanced Architectures
//
//   All rights reserved
//   Confidential Information
//   Limited Distribution to authorized Persons Only
//   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//   Project Name         :  A2S
//
//   Description          :  Address Definitions for A2S I/O and Tile Accesses
//
//================================================================================================================================
//
//   THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//
//================================================================================================================================
//   Notes:
//
//
//================================================================================================================================

`ifndef A2S_REGISTERS
`define A2S_REGISTERS

// A2S Register Base Addresses
`define STATUS_base         	 13'h0000
`define SPFPU_base          	 13'h0080
`define SIMD_base           	 13'h0090

// A2S Number of Registers 
`define STATUS_range        	 10;
`define SPFPU_range         	 2;
`define SIMD_range          	 6;

// A2S Register Indices
`define iST                 	 0
`define iRS                 	 1
`define iDS                 	 2
`define iIN                 	 3
`define iIE                 	 4
`define iEP                 	 5
`define iRP                 	 6
`define iRC                 	 7
`define iDP                 	 8
`define iDC                 	 9

`define iFCS                	 0
`define iFRM                	 1

`define iWP                 	 0
`define iXP                 	 1
`define iYP                 	 2
`define iZP                 	 3
`define iCT                 	 4
`define iCF                 	 5

// A2S Register Fields
// Fields for ST
`define ST_IEN	ST[0] 		// Interrupt Enable
`define ST_SVR	ST[1] 		// Supervisor Mode
`define ST_PFC	ST[2] 		// Prefetch Caught
`define ST_WFI	ST[3] 		// Wait For Interrupt

// Fields for RS
`define RS_STH	RS[9:0]	// Spill Threshold
`define RS_FTH	RS[19:10]	// Fill Threshold
`define RS_STD	RS[30:20]	// Depth
`define RS_SMT	RS[31] 		// Empty

// Fields for DS
`define DS_STH	DS[9:0]	// Spill Threshold
`define DS_FTH	DS[19:10]	// Fill Threshold
`define DS_STD	DS[30:20]	// Depth
`define DS_SMT	DS[31] 		// Empty

// Fields for IN
`define IN_POR	IN[0] 		// Power On Reset
`define IN_RUF	IN[1] 		// R stack underflow
`define IN_DUF	IN[2] 		// D stack undwerflow
`define IN_RFL	IN[3] 		// R stack full
`define IN_DFL	IN[4] 		// D stack full
`define IN_RSL	IN[5] 		// R stack spill
`define IN_DSL	IN[6] 		// D stack spill
`define IN_RFI	IN[7] 		// R stack fill
`define IN_DFI	IN[8] 		// D stack fill
`define IN_FPX	IN[9] 		// Floating Point Exception -- if installed
`define IN_USR	IN[30] 		// User
`define IN_EXI	IN[31] 		// External

// Fields for IE
`define IE_POR	IE[0] 		// Power On Reset - always enabled!
`define IE_RUF	IE[1] 		// R stack underflow
`define IE_DUF	IE[2] 		// D stack undwerflow
`define IE_RFL	IE[3] 		// R stack full
`define IE_DFL	IE[4] 		// D stack full
`define IE_RSL	IE[5] 		// R stack spill
`define IE_DSL	IE[6] 		// D stack spill
`define IE_RFI	IE[7] 		// R stack fill
`define IE_DFI	IE[8] 		// D stack fill
`define IE_FPX	IE[9] 		// Floating Point Exception -- if installed
`define IE_USR	IE[30] 		// User
`define IE_EXI	IE[31] 		// External

// Fields for EP
`define EP_EMT	EP[1:0]	// pad zeros;  32-bit aligned
`define EP_EIN	EP[6:2]	// encoded interrupt
`define EP_EPB	EP[31:7]	// entry point base


`endif
